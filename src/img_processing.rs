use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{Cursor};
use std::process::Command;
use std::num::NonZeroU32;
use std::time::Instant;
use fast_image_resize as fr;
use fr::images::Image;


/////////////////
/////FIND RAWS
////////////////

pub fn find_raw_files(folder: &Path) -> std::io::Result<Vec<PathBuf>> {
    let read_dir = std::fs::read_dir(folder)?;
    
    let mut paths: Vec<PathBuf> = read_dir
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            if !path.is_file() { return false; }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                matches!(
                    ext.to_ascii_lowercase().as_str(),
                    "cr2" | "cr3" | "crm" | "crw" | "nef" | "nrw" | "arw" | "srf" | "sr2" |
                    "raf" | "dng" | "rw2" | "orf" | "ori" | "pef" | "rwl" | "3fr" | "fff" |
                    "srw" | "bay" | "mef" | "mos" | "mrw" | "ptx" | "pxn" | "r3d" | "raw" |
                    "rwz" | "x3f"
                )
            } else {
                false
            }
        })
        .collect();

    // ── THE SORT ENGINE ──────────────────────────────────────────────────────
    // Sorts the collected files by their creation time (Oldest to Newest).
    // Swap `a` and `b` at the end (`b_time.cmp(&a_time)`) if you want Newest first!
    paths.sort_by(|a, b| {
        let a_time = fs::metadata(a)
            .and_then(|m| m.created().or_else(|_| m.modified()))
            .unwrap_or_else(|_| std::time::SystemTime::now());

        let b_time = fs::metadata(b)
            .and_then(|m| m.created().or_else(|_| m.modified()))
            .unwrap_or_else(|_| std::time::SystemTime::now());

        a_time.cmp(&b_time) 
    });

    Ok(paths)
}


pub struct ExtractedPreview {
    pub jpeg_bytes: Vec<u8>,
    pub orientation: u16,
}

/////////////////
/////PARSE RAWS
////////////////

pub fn parse_raw_preview(path: &Path) -> Option<ExtractedPreview> {
    let file_name = path.file_name()?.to_string_lossy();
    println!("[START] Processing image: {}", file_name);
    
    // Start measuring total execution time for this asset
    let start_time = Instant::now();

    let exiftool_executable = crate::exif_process::ensure_embedded_exiftool().ok()?;
    let temp_dir = std::env::temp_dir().join("dnglab_engine_v1");

    // Pass 1: Try -PreviewImage tag extraction
    let output = Command::new("perl")
        .env("PERL5LIB", temp_dir.join("lib"))
        .arg(&exiftool_executable)
        .arg("-b")
        .arg("-PreviewImage")
        .arg(path)
        .output()
        .ok()?;

    let mut jpeg_bytes = output.stdout;
    let mut fallback_used = false;

    // Pass 2 Fallback: If primary pass yielded no data, run secondary tag validation
    if jpeg_bytes.is_empty() || jpeg_bytes.len() < 1000 {
        println!("[WARN] Primary '-PreviewImage' missing or insufficient for {}. Initiating secondary pass fallback...", file_name);
        fallback_used = true;

        let fallback_output = Command::new("perl")
            .env("PERL5LIB", temp_dir.join("lib"))
            .arg(&exiftool_executable)
            .arg("-b")
            .arg("-JpgFromRaw")
            .arg(path)
            .output()
            .ok()?;
        jpeg_bytes = fallback_output.stdout;
    }

    if jpeg_bytes.is_empty() || jpeg_bytes.len() < 1000 {
        eprintln!("[ERROR] Embedded ExifTool failed to acquire preview bytes for {} on all passes.", file_name);
        return None;
    }

    if fallback_used {
        println!("[SUCCESS] Preview image acquired via secondary pass fallback (-JpgFromRaw) for {}.", file_name);
    } else {
        println!("[SUCCESS] Preview image acquired successfully via primary pass (-PreviewImage) for {}.", file_name);
    }

    // Read EXIF orientation safely from the raw file memory map
    let file = File::open(path).ok()?;
    let mmap = unsafe { memmap2::Mmap::map(&file).ok()? };
    let data = &mmap[..];

    let mut orientation = 1u16;
    let mut cursor = Cursor::new(data);
    if let Ok(exif_data) = exif::Reader::new().read_from_container(&mut cursor) {
        if let Some(field) = exif_data.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
            if let exif::Value::Short(ref v) = field.value {
                if !v.is_empty() { orientation = v[0]; }
            }
        }
    }

    let elapsed = start_time.elapsed();
    println!("[END] Finished parsing execution pipeline for {} in {}ms.", file_name, elapsed.as_millis());

    Some(ExtractedPreview { jpeg_bytes, orientation })
}


/////////////////
/////JPEG FROM RAWS FOR THUMBNAILS AND PREVIEWS
////////////////

pub fn save_jpeg_with_exif(
    raw_path: &Path, 
    preview: ExtractedPreview, 
    target_size: u32
) -> Result<(PathBuf, u16), Box<dyn std::error::Error>> {
    let file_name = raw_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let resize_start = Instant::now();

    let processed_bytes = resize_jpeg_payload(&preview.jpeg_bytes, target_size, preview.orientation)
        .map_err(|e| {
            eprintln!("[ERROR] Failed to resize image {}: {}", file_name, e);
            e
        })?;

    let mut cache_path = crate::manage_cache::obtener_ruta_cache()?;
    std::fs::create_dir_all(&cache_path)?;

    let mut jpg_name = PathBuf::from(&file_name);
    if target_size > 300 {
        if let Some(file_stem) = raw_path.file_stem() {
            let mut new_name = file_stem.to_os_string();
            new_name.push("_preview.jpg");
            jpg_name = PathBuf::from(new_name);
        }
    } else {
        jpg_name.set_extension("jpg");
    }
    cache_path.push(jpg_name);

    std::fs::write(&cache_path, &processed_bytes)?;
    
    println!(
        "[CACHE] Saved downscaled preview to {:?} (Resized & Rotated in {}ms)", 
        cache_path.file_name().unwrap_or_default(),
        resize_start.elapsed().as_millis()
    );

    Ok((cache_path, preview.orientation))
}


/////////////////
/////RESIZE AND ROTATE
////////////////


fn resize_jpeg_payload(
    input_bytes: &[u8], 
    target_final_width: u32, 
    orientation: u16
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut img = image::load_from_memory_with_format(input_bytes, image::ImageFormat::Jpeg)?;

    img = match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    };

    let rgb_img = img.to_rgb8();
    let src_width = NonZeroU32::new(rgb_img.width()).ok_or("Invalid source width")?;
    let src_height = NonZeroU32::new(rgb_img.height()).ok_or("Invalid source height")?;

    let src_image = Image::from_vec_u8(
        src_width.get(),
        src_height.get(),
        rgb_img.into_raw(),
        fr::PixelType::U8x3,
    )?;

    let dst_width = target_final_width;
    let dst_height = (target_final_width * src_height.get()) / src_width.get();
    
    let dst_width_nz = NonZeroU32::new(dst_width).ok_or("Invalid target width")?;
    let dst_height_nz = NonZeroU32::new(dst_height.max(1)).ok_or("Invalid target height")?;

    let mut dst_image = Image::new(
        dst_width_nz.get(),
        dst_height_nz.get(),
        src_image.pixel_type(),
    );

    let mut resizer = fr::Resizer::new();
    resizer.resize(&src_image, &mut dst_image, None)?;

    let resized_raw = dst_image.into_vec();
    let resized_buffer = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_raw(
        dst_width_nz.get(),
        dst_height_nz.get(),
        resized_raw
    ).ok_or("Failed to rebuild image buffer")?;

    let mut resized_jpeg_bytes = Vec::new();
    let mut cursor = Cursor::new(&mut resized_jpeg_bytes);
    
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 90);
    resized_buffer.write_with_encoder(encoder)?;

    Ok(resized_jpeg_bytes)
}





