use std::path::{Path, PathBuf};
use std::fs::{self};
use std::io::{Cursor};
use fast_image_resize as fr;
use log::{info, error};
use image::{ImageReader, ImageFormat};


// In this file
//
//1) Find raw images in folder
//2) Resize jpeg image
//3) Rotate jpeg image
//
//


/////////////////
/////1) FIND RAWS
////////////////
//Scans a flat directory for common RAW image extensions and returns their paths.

pub fn discover_raw_images(dir_path: &Path) -> Result<Vec<PathBuf>, String> {
    info!("[Engine] Scanning directory for RAW images: {}", dir_path.display());

    if !dir_path.is_dir() {
        let err = format!("Provided path is not a directory: {}", dir_path.display());
        error!("{}", err);
        return Err(err);
    }

    // Common standard RAW image format extensions
    let raw_extensions = [
        "cr2", "cr3", "crm", "crw", "nef", "nrw", "arw", "srf", "sr2", 
        "raf", "dng", "rw2", "orf", "ori", "pef", "rwl", "3fr", "fff", 
        "srw", "bay", "mef", "mos", "mrw", "ptx", "pxn", "r3d", "raw", 
        "rwz", "x3f"
    ];
    let mut discovered_raws = Vec::new();

    let entries = fs::read_dir(dir_path).map_err(|e| {
        let err = format!("Failed to read directory entries: {e}");
        error!("{}", err);
        err
    })?;

    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if raw_extensions.contains(&ext.to_lowercase().as_str()) {
                        discovered_raws.push(path);
                    }
                }
            }
        }
    }

    info!("[Engine] Found {} RAW images in {}", discovered_raws.len(), dir_path.display());
    Ok(discovered_raws)
}

////////////
/////2. RESIZE
/////////////
/// Given a path to a JPEG file and a target width, resizes the image 
/// preserving its aspect ratio, returning the new JPEG bytes.


pub fn resize_jpeg_by_width(jpeg_bytes: &[u8], target_width: u32) -> Result<Vec<u8>, String> {
    info!("[Engine] Resizing in-memory JPEG byte stream to target width: {}px", target_width);

    if jpeg_bytes.is_empty() {
        let err = "Input image byte stream is empty.".to_string();
        error!("{}", err);
        return Err(err);
    }

    if target_width == 0 {
        let err = "Target width must be greater than 0 pixels.".to_string();
        error!("{}", err);
        return Err(err);
    }

    // 1. Open and decode the image directly from the in-memory byte slice
    let mut reader = ImageReader::new(Cursor::new(jpeg_bytes));
    reader.set_format(ImageFormat::Jpeg);
    
    let img = reader.decode().map_err(|e| {
        let err = format!("Decode from memory failed: {}", e);
        error!("{}", err);
        err
    })?;

    let src_w = img.width();
    let src_h = img.height();

    // 2. Calculate the matching target height to preserve the aspect ratio
    let aspect_ratio = src_h as f64 / src_w as f64;
    let target_height = ((target_width as f64 * aspect_ratio) as u32).max(1);

    info!("[Engine] Aspect ratio calculated. Scaling from {}x{} to {}x{}", src_w, src_h, target_width, target_height);

    // Convert dynamic image frame buffer down to uniform RGBA bytes vector in-scope
    let rgba_buffer = img.to_rgb8().into_raw();

    // 3. Setup fast_image_resize buffers
    let src_image = fr::images::Image::from_vec_u8(
        src_w,
        src_h,
        rgba_buffer,
        fr::PixelType::U8x3,
    ).map_err(|e| format!("Failed to create source image buffer: {}", e))?;
    
    let mut dst_image = fr::images::Image::new(
        target_width,   
        target_height,  
        fr::PixelType::U8x3,
    );

    let mut resizer = fr::Resizer::new();
    resizer.resize(&src_image, &mut dst_image, None).map_err(|e| {
        let err = format!("SIMD resize processing filter failed: {e}");
        error!("{}", err);
        err
    })?;

    // 4. Re-encode the resized buffer back into standard JPEG bytes
    let mut encoded_buffer = Vec::new();
    let dest_rgba_buffer = dst_image.buffer();
    
    image::codecs::jpeg::JpegEncoder::new(&mut encoded_buffer)
        .encode(dest_rgba_buffer, target_width, target_height, image::ExtendedColorType::Rgb8)
        .map_err(|e| {
            let err = format!("Failed to re-encode resized pixel data to JPEG: {e}");
            error!("{}", err);
            err
        })?;

    info!("[Engine] Successfully resized JPEG data to width {}px (Total size: {} bytes).", target_width, encoded_buffer.len());
    Ok(encoded_buffer)
}
///////////
///3. ROTATE
///////////

/// Given a path to a JPEG file and an Exif orientation string, 
/// rotates the image accordingly and returns the modified JPEG bytes.


pub fn rotate_jpeg_bytes(jpeg_bytes: &[u8], orientation: &str) -> Result<Vec<u8>, String> {
    info!("[Engine] Rotating in-memory JPEG bytes based on orientation: {}", orientation);

    if jpeg_bytes.is_empty() {
        return Err("Input JPEG bytes are empty.".to_string());
    }

    // 1. Decode image from the memory slice
    let mut reader = ImageReader::new(Cursor::new(jpeg_bytes));
    reader.set_format(ImageFormat::Jpeg);
    let img = reader.decode().map_err(|e| {
        let err = format!("Failed to decode JPEG bytes for rotation: {e}");
        error!("{}", err);
        err
    })?;

    // 2. Perform rotation transformation based on the Exif string
    let rotated_img = match orientation {
        "Rotate 90 CW" | "90 CW" => img.rotate90(),
        "Rotate 180" | "180" => img.rotate180(),
        "Rotate 270 CW" | "270 CW" | "90 CCW" => img.rotate270(),
        _ => img, // Horizontal (normal) or unknown needs no change
    };

    // 3. Re-encode the rotated frame back into standard JPEG bytes
    let mut encoded_buffer = Vec::new();
    
    // We can use the dynamic image's save encoder wrapper or direct JPEG encoder.
    // Using rgb8 mapping here ensures compatibility with standard JPEG specs.
    let rgb_buffer = rotated_img.to_rgb8();
    image::codecs::jpeg::JpegEncoder::new(&mut encoded_buffer)
        .encode(
            &rgb_buffer, 
            rotated_img.width(), 
            rotated_img.height(), 
            image::ExtendedColorType::Rgb8
        )
        .map_err(|e| {
            let err = format!("Failed to re-encode rotated JPEG: {e}");
            error!("{}", err);
            err
        })?;

    Ok(encoded_buffer)
}