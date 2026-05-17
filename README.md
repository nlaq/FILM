# DGNFilm
Little app to lossless compress raw image files (arw, cr3, nef, etc.) and convert DNG. Useful for users of cameras without lossless compression (like the Sony a7r II and a7r III or Leica SL, SL 2), or for those who just want to keep their images in DNG format.

<img width="382" height="437" alt="screenshot" src="https://github.com/user-attachments/assets/8d63d281-75af-45a3-9b65-85f9f3b59073" />

It works on linux and Mac Os. If you dowload a release [https://github.com/dnglab/DNGFilm/releases] just double click on the file to open the app. 

On Mac OS 

You may probably nedd to give permissions settings, privacy and security. After that you can copy the app file to your applications folder.

On linux 

Copy the .desktop file to ~/.local/share/applications/ or /usr/share/applications/ and the release binary to ~/.local/bin or /usr/bin 

Build

To compile the app, just use cargo build.

Thanks to DNGLab, an Exiftool.
