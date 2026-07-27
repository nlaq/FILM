# Film
Little app to lossless compress raw image files (arw, cr3, nef, etc.) and convert them to DNG. It also let you write exif lens and aperture data, for those using vintange lenses. It does not support lastest nef and cr3 privative compression algorithms. Useful for users of cameras without lossless compression (like the Sony a7r II and a7r III or Leica SL, SL 2), or for those who just want to keep their images in DNG format.

<img width="1103" height="739" alt="screen" src="https://github.com/user-attachments/assets/e66ad927-8a0f-439f-b878-144ad5f279dd" />

It works on Linux and Mac Os. If you dowload a release [https://github.com/nlaq/Film/releases] just double click on the file to open the app. 

**On Mac OS**

The release is for apple silicon, you may probably need to give permissions: settings, privacy and security. After that you can copy the app file to your applications folder.

**On Linux**

Only AArch64. AppImage file, download, give permission to execute and double click.

**Build**

For other platforms (x86, etc) compile the app. Install cargo, go to the code folder on terminal: cargo build --release.

**Use**

To add/remove lenses and config dng convertion, check the edit menu.

Thanks to DNGLab, an Exiftool.
