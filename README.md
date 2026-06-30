# Film
Little app to lossless compress raw image files (arw, cr3, nef, etc.) and convert them to DNG. It also let you write exif lens and aperture data, for those using vintange lenses. It does not support lastest nef and cr3 privative compression algorithms. Useful for users of cameras without lossless compression (like the Sony a7r II and a7r III or Leica SL, SL 2), or for those who just want to keep their images in DNG format.

<img width="1103" height="739" alt="screen" src="https://github.com/user-attachments/assets/e66ad927-8a0f-439f-b878-144ad5f279dd" />

It works on Linux and Mac Os. If you dowload a release [https://github.com/nlaq/Film/releases] just double click on the file to open the app. 

**On Mac OS**

You may probably nedd to give permissions settings, privacy and security. After that you can copy the app file to your applications folder.

**On Linux**

Copy the .desktop file to ~/.local/share/applications/ or /usr/share/applications/ and the release binary to ~/.local/bin or /usr/bin 

**Build**

To compile the app, just use cargo build.

Thanks to DNGLab, an Exiftool.
