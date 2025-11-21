use axum::{
    extract::{Form},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use askama::Template;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tower_http::services::ServeDir;
use std::fs;

// --- Data Structures ---

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct YtDlpFormat {
    format_id: String,
    ext: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    acodec: Option<String>,
    vcodec: Option<String>,
    filesize: Option<u64>,
    filesize_approx: Option<u64>,
    language: Option<String>,
    format_note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct YtDlpOutput {
    title: String,
    formats: Vec<YtDlpFormat>,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "analyze.html")]
struct AnalyzeTemplate {
    url: String,
    title: String,
    formats: Vec<DisplayFormat>,
    languages: Vec<String>,
}

#[derive(Debug, PartialEq)]
enum MediaType {
    Video,
    Audio,
    Other,
}

#[derive(Debug)]
struct FileInfo {
    name: String,
    media_type: MediaType,
    mime_type: String,
    size_mb: String,
}

#[derive(Template)]
#[template(path = "file_list.html")]
struct FileListTemplate {
    files: Vec<FileInfo>,
}

#[derive(Debug, Clone)]
struct DisplayFormat {
    id: String,
    ext: String,
    resolution: String,
    filesize: String,
    codecs: String,
    language: String,
    type_label: String,
    raw_height: u32,
}

#[derive(Deserialize)]
struct AnalyzeRequest {
    url: String,
}

// Updated struct to accept file_type
#[derive(Deserialize)]
struct DownloadRequest {
    url: String,
    format_id: String,
    file_type: String, 
}

// --- Main ---

#[tokio::main]
async fn main() {
    let _ = fs::create_dir_all("downloads");
    let _ = fs::create_dir_all("assets");

    let app = Router::new()
        .route("/", get(show_index))
        .route("/analyze", post(analyze_url))
        .route("/download", post(download_format))
        .route("/files", get(show_files))
        .nest_service("/assets", ServeDir::new("assets"))
        .nest_service("/content", ServeDir::new("downloads"));

    println!("Server running on http://localhost:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// --- Handlers ---

async fn show_index() -> impl IntoResponse {
    IndexTemplate { error: None }
}

async fn analyze_url(Form(input): Form<AnalyzeRequest>) -> impl IntoResponse {
    let output = Command::new("./yt-dlp_linux")
        .arg("--dump-json")
        .arg(&input.url)
        .output();

    match output {
        Ok(o) => {
            if !o.status.success() {
                let err_msg = String::from_utf8_lossy(&o.stderr).to_string();
                return IndexTemplate { error: Some(format!("yt-dlp error: {}", err_msg)) }.into_response();
            }

            let json_str = String::from_utf8_lossy(&o.stdout);
            let meta: YtDlpOutput = match serde_json::from_str(&json_str) {
                Ok(m) => m,
                Err(_) => return IndexTemplate { error: Some("Failed to parse JSON from yt-dlp".to_string()) }.into_response(),
            };

            let mut display_formats = Vec::new();
            let mut languages = Vec::new();

            for f in meta.formats {
                let is_audio = f.acodec.as_deref().unwrap_or("none") != "none";
                let is_video = f.vcodec.as_deref().unwrap_or("none") != "none";

                let type_label = if is_audio && is_video {
                    "Video+Audio"
                } else if is_video {
                    "Video Only"
                } else {
                    "Audio Only"
                };

                let size = f.filesize.or(f.filesize_approx).unwrap_or(0);
                let size_str = if size > 0 {
                    format!("{:.2} MB", size as f64 / 1024.0 / 1024.0)
                } else {
                    "Unknown".to_string()
                };

                let res = if let (Some(w), Some(h)) = (f.width, f.height) {
                    format!("{}x{}", w, h)
                } else {
                    "Audio".to_string()
                };

                let lang = f.language.clone().unwrap_or_else(|| "Unknown".to_string());
                if lang != "Unknown" && !languages.contains(&lang) {
                    languages.push(lang.clone());
                }

                display_formats.push(DisplayFormat {
                    id: f.format_id,
                    ext: f.ext.unwrap_or_default(),
                    resolution: res,
                    filesize: size_str,
                    codecs: format!("{}/{}", f.vcodec.unwrap_or("none".into()), f.acodec.unwrap_or("none".into())),
                    language: lang,
                    type_label: type_label.to_string(),
                    raw_height: f.height.unwrap_or(0),
                });
            }

            languages.sort();

            Html(AnalyzeTemplate { 
                url: input.url, 
                title: meta.title, 
                formats: display_formats,
                languages 
            }.render().unwrap()).into_response()
        }
        Err(e) => IndexTemplate { error: Some(e.to_string()) }.into_response(),
    }
}

async fn download_format(Form(req): Form<DownloadRequest>) -> impl IntoResponse {
    let mut cmd = Command::new("./yt-dlp_linux");
    
    // Logic: If Audio Only, convert to MP3. If Video, merge to MP4.
    if req.file_type == "Audio Only" {
        cmd.arg("-f")
           .arg(&req.format_id)
           .arg("-x")                  // Extract audio
           .arg("--audio-format")      // Convert to...
           .arg("mp3")                 // ...mp3
           .arg("-o")
           .arg("downloads/%(title)s.%(ext)s")
           .arg(&req.url);
    } else {
        // Video logic
        cmd.arg("-f")
           .arg(&req.format_id)
           .arg("--merge-output-format")
           .arg("mp4")
           .arg("-o")
           .arg("downloads/%(title)s.%(ext)s")
           .arg(&req.url);
    }

    let status = cmd.status();

    match status {
        Ok(s) if s.success() => Redirect::to("/files").into_response(),
        _ => Html("<h1>Download Failed</h1><a href='/'>Go Back</a>".to_string()).into_response(),
    }
}

async fn show_files() -> impl IntoResponse {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir("downloads") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(name) = entry.file_name().into_string() {
                    if !name.starts_with(".") {
                        let mime = mime_guess::from_path(&path).first_or_octet_stream();
                        let mime_str = mime.to_string();
                        
                        let media_type = if mime.type_() == "video" {
                            MediaType::Video
                        } else if mime.type_() == "audio" {
                            MediaType::Audio
                        } else {
                            MediaType::Other
                        };

                        let metadata = fs::metadata(&path).ok();
                        let size = metadata.map(|m| m.len()).unwrap_or(0);
                        let size_mb = format!("{:.2} MB", size as f64 / 1024.0 / 1024.0);

                        files.push(FileInfo {
                            name,
                            media_type,
                            mime_type: mime_str,
                            size_mb
                        });
                    }
                }
            }
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Html(FileListTemplate { files }.render().unwrap()).into_response()
}