# Camera still-picture input

Check `capabilities().camera.hostPicture` and `formats`. On an implemented camera board,
load firmware first, then call `setCameraPicture({bytes: Uint8Array | ArrayBuffer})` with
a BMP, binary PPM, or a supported baseline JPEG, at most 8 MiB. The result acknowledges
only the sensor input; firmware must configure SCCB/XCLK and capture through I2S or
LCD_CAM DMA. Check guest RAM/serial or its application display to verify consumption.
The still image is private to the current Worker; it is excluded from shares and
recordings and must be selected again after the Worker is replaced. No webcam, optics,
physical camera, manufacturer firmware or timing acceptance follows from this API.
The workbench and mini expose the same explicit image chooser and iframe API.
