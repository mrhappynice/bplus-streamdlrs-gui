# bplus🤷🏻‍♂️ - streamdlrs-gui

- download and view videos or audio
- download from server to device

to install:
- sex x on files:
```sh
chmod +x cp-app.sh get-dlp.sh
```
- get yt-dlp:
```sh
./get-dlp.sh
```
- build the app:
```sh
cargo build
```
- if ffmpeg not installed, do it:
```sh
sudo apt update && sudo apt install ffmpeg
```

to run:
- put yt-dlp_linux in the local folder(get-dlp.sh does this)
- have ffmpeg installed
- run the app and browse to http://localhost:3000
```sh
./bplus-streamdlrs-gui
```

Ubuntu 22+ compiled binary in releases
