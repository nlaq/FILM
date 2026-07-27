use std::path::{Path, PathBuf};
use std::fs::{self};
use std::io::{Cursor};
use std::process::{Command};
use regex::Regex;
use log::{info, error, warn};
use crate::models::CameraMetadata;



#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;


//In this file
//0) execute exiftool
//1) Extracts Exif metadata from a RAW image file.
//2) Extracts the embedded JPG preview image from a RAW file as a byte vector.
//3) Injects all metadata tags (including previews/thumbnails)
//4) Injects specific custom Lens Make and Lens Model
//5) Injects a specific lens aperture value
//6) Orientation of an image
//7) Injects a specific focal length
//8) Define lens id
//9) INJECTS EXTRACTED PREVIEW BYTES INTO THE SIMPLEST ROOT LEVEL OF A DNG




// Link directly to your zipped resource file
const EXIFTOOL_ZIP: &[u8] = include_bytes!("../bin/exiftool.zip");

/// Safely unpacks your embedded zip to the temp directory and returns the script path
pub fn ensure_embedded_exiftool() -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir().join("dnglab_engine_v1");
    let exiftool_executable = temp_dir.join("exiftool");
    
    // A marker file guarantees extraction finished completely and successfully last time
    let success_marker = temp_dir.join(".extraction_success");

    if !exiftool_executable.exists() || !success_marker.exists() {
        println!("[INFO] Extracting embedded ExifTool environment to temporary directory...");
        
        // Clean up any failed previous extraction attempts safely
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {e}"))?;
        
        let reader = Cursor::new(EXIFTOOL_ZIP);
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            
            // Mitigate Zip Slip vulnerability using enclosed_name
            let enclosed_path = file.enclosed_name()
                .ok_or_else(|| "Invalid file path in zip archive".to_string())?;
            let outpath = temp_dir.join(enclosed_path);

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
            } else {
                if let Some(p) = outpath.parent() { 
                    fs::create_dir_all(p).map_err(|e| e.to_string())?; 
                }
                
                let mut outfile = fs::File::create(&outpath)
                    .map_err(|e| format!("Failed to create output file {}: {}", outpath.display(), e))?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                
                // Assign executable POSIX permissions safely only on Unix platforms
                #[cfg(unix)]
                {
                    if let Ok(metadata) = fs::metadata(&outpath) {
                        let mut perms = metadata.permissions();
                        perms.set_mode(0o755);
                        let _ = fs::set_permissions(&outpath, perms);
                    }
                }
            }
        }
        
        // Create the success marker file after everything extracts cleanly
        fs::File::create(&success_marker).map_err(|e| format!("Failed to create success marker: {e}"))?;
        println!("[INFO] ExifTool environment successfully extracted and prepared.");
    }

    Ok(exiftool_executable)
}


//////////LENS BRANDS

pub const KNOWN_LENS_BRANDS: &[&str] = &[
    "Canon", "Nikon", "Sony", "Sigma", "Tamron",
    "Fujifilm", "Panasonic", "Olympus", "Leica",
    "Voigtländer", "Voigtlander", "Minolta",
    "Pentax", "Konica", "Irix", "Lomography",
    "Pentacon", "Viltrox", "7artisans",
    "TTArtisan", "Laowa", "Samyang",
    "Rokinon", "Tokina", "Zeiss", "Rollei",
    "Pergear", "Zhong Yi", "Zhongyi",
    "Mitakon", "Brightin Star", "Sirui",
    "AstrHori", "Meike", "Thypoch",
    "Light Lens Lab", "Mr. Ding",
    "MS Optics", "MS-Optics",
    "Artizlab", "DJ-Optical",
    "Omnar", "Funleader", "Kamlan",
    "Neewer", "Dulens",
    "Helios", "Jupiter", "Zenit",
    "Industar", "Mir", "Tair",
    "Meyer-Optik", "Meyer Optik",
    "Görlitz", "Goerlitz",
    "Schneider-Kreuznach", "Enna",
];

//////////////////////////////////////////////////////
/// SINGLE EXIFTOOL WRITE EXECUTOR
//////////////////////////////////////////////////////

pub fn execute_exiftool_write(
    exiftool_bin: &Path,
    destination_path: &Path,
    mut args: Vec<String>,
) -> Result<(), String> {

    if !destination_path.exists() {
        let err = format!(
            "Destination image does not exist: {}",
            destination_path.display()
        );

        error!("{}", err);
        return Err(err);
    }

    args.push(
        "-overwrite_original".to_string()
    );

    args.push(
        destination_path
            .to_string_lossy()
            .to_string()
    );

    info!(
        "[ExifTool] Executing metadata write on {}",
        destination_path.display()
    );

    let output = Command::new(exiftool_bin)
        .args(&args)
        .output()
        .map_err(|e| {

            let err = format!(
                "Failed executing ExifTool: {}",
                e
            );

            error!("{}", err);

            err
        })?;

    if !output.status.success() {

        let stderr =
            String::from_utf8_lossy(
                &output.stderr
            );

        let stdout =
            String::from_utf8_lossy(
                &output.stdout
            );


        let message =
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            };

        let err = format!(
            "ExifTool failed: {}",
            message
        );

        error!("{}", err);
        return Err(err);
    }

    info!(
        "[ExifTool] Metadata write completed successfully"
    );

    Ok(())
}





// --- Core Functions ---
/////////////////////
/// 1) Extracts Exif metadata from a RAW image file.
///////////////////



pub fn extract_metadata(
    exiftool_bin: &Path, 
    image_path: &Path,
    tags: &[&str], // Takes any list of tags dynamically (e.g., &["-Model", "-Make"])
) -> Result<CameraMetadata, String> {
    info!("[ExifTool] Extracting metadata from: {}", image_path.display());

    if !image_path.exists() {
        let err = format!("Input image file does not exist: {}", image_path.display());
        error!("{}", err);
        return Err(err);
    }

    // Build the arguments dynamically
    let mut args = vec!["-j"]; // Start with the JSON flag
    
    // Append all the requested tags
    args.extend_from_slice(tags);
    
    // Append the file path at the very end
    args.push(image_path.to_str().unwrap_or(""));

    let output = Command::new(exiftool_bin)
        .args(&args)
        .output()
        .map_err(|e| {
            let err = format!("Failed to execute ExifTool: {e}");
            error!("{}", err);
            err
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err = format!("ExifTool exited with error: {}", stderr.trim());
        error!("{}", err);
        return Err(err);
    }

    // ExifTool JSON output is always an array: [{ ... }]
    let metadata_list: Vec<CameraMetadata> = serde_json::from_slice(&output.stdout)
        .map_err(|e| {
            let err = format!("Failed to parse ExifTool JSON metadata: {e}");
            error!("{}", err);
            err
        })?;

    let metadata = metadata_list.into_iter().next().ok_or_else(|| {
        let err = "ExifTool returned an empty JSON array".to_string();
        error!("{}", err);
        err
    })?;

    info!("[ExifTool] Successfully extracted metadata for {}", image_path.display());
    Ok(metadata)
}


/////////////////////////
/// 2) Extracts the embedded JPG preview image from a RAW file as a byte vector.
/////////////////////////

pub fn extract_preview_image(exiftool_bin: &Path, image_path: &Path) -> Result<Vec<u8>, String> {
    info!("[ExifTool] Extracting preview image from: {}", image_path.display());

    if !image_path.exists() {
        let err = format!("Input image file does not exist: {}", image_path.display());
        error!("{}", err);
        return Err(err);
    }

    // -PreviewImage -b extracts the binary payload of the embedded preview directly to stdout
    let output = Command::new(exiftool_bin)
        .args([
            "-b",
            "-MPImage", 
            "-JpgFromRaw", 
            "-PreviewImage", 
            image_path.to_str().unwrap_or("")
        ])
        .output() // 👈 Asegúrate de que termine en .output()
        .map_err(|e| { // 👈 Y encadena el mapeo del error correctamente
            let err = format!("Failed to execute ExifTool binary: {e}");
            error!("{}", err);
            err
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err = format!("ExifTool preview extraction failed: {}", stderr.trim());
        error!("{}", err);
        return Err(err);
    }

    if output.stdout.is_empty() {
        let err = format!("No preview image found inside: {}", image_path.display());
        warn!("{}", err);
        return Err(err);
    }

    info!("[ExifTool] Preview extracted completely ({} bytes).", output.stdout.len());
    Ok(output.stdout)
}




//////////////////////////////////////////////////////
/// 3. COPY ALL METADATA FROM ORIGINAL RAW
///
/// Copies:
/// - EXIF
/// - MakerNotes
/// - XMP
/// - IPTC
/// - previews
/// - thumbnails
//////////////////////////////////////////////////////


pub fn inject_all_tags_args(
    source_path: &Path,
) -> Vec<String> {
    vec![
        "-TagsFromFile".to_string(),
        source_path.to_string_lossy().into_owned(),
        
        // Copy general text metadata tags
        "-all:all".to_string(),
        "-unsafe".to_string(),
        
                
        // FIX 1: Corrected Preview and Image Exclusions
        "--ThumbnailImage".to_string(),
        "--PreviewImage".to_string(),
        "--JpgFromRaw".to_string(),
        "--OtherImage".to_string(),
        
        // FIX 2: Corrected Group & Directory Protections
        "--Adobe:*".to_string(), 
        "--DNG:*".to_string(),
        "--SubIFD:*".to_string(), // Crucial: Prevents copying source SubIFD directories entirely
        
        // 2. GEOMETRIC AND DIMENSION PROTECTION (Your excellent list)
        "--ImageWidth".to_string(),
        "--ImageHeight".to_string(),
        "--BitsPerSample".to_string(),
        "--Compression".to_string(),
        "--DefaultCropOrigin".to_string(),
        "--DefaultCropSize".to_string(),
        "--ActiveArea".to_string(),
        
        // 3. COLOR, MOSAIC, AND COLOR SCIENCE PROTECTION
        "--BlackLevel".to_string(),
        "--WhiteLevel".to_string(),
        "--CFASelection".to_string(),
        "--CFAPattern".to_string(),
                
        // FIX 3: Added critical DNG Color Matrix Protections
        "--ColorMatrix*".to_string(),
        "--ForwardMatrix*".to_string(),
        "--AsShotNeutral".to_string(), // Protects the custom white balance array
        
        // 4. ROTATION PROTECTION
        "--Orientation".to_string(),
        "--*Orientation*".to_string(), 
        "--Rotation".to_string(),

                
        // Tell ExifTool to edit the destination file directly on disk
        "-overwrite_original".to_string(),
    ]
}

//////////////////////////////////////////////////////
/// LENS MAKE
//////////////////////////////////////////////////////

pub fn inject_lens_make_args(
    lens_make: &str,
) -> Vec<String> {


    vec![

        format!(
            "-LensMake={}",
            lens_make
        )

    ]
}


//////////////////////////////////////////////////////
/// LENS MODEL
//////////////////////////////////////////////////////

pub fn inject_lens_model_args(
    lens_model: &str,
) -> Vec<String> {


    vec![

        format!(
            "-LensModel={}",
            lens_model
        )

    ]
}





//////////////////////////////////////////////////////
/// APERTURE
//////////////////////////////////////////////////////

pub fn inject_aperture_metadata_args(
    aperture: f64,
) -> Result<Vec<String>, String> {


    if aperture <= 0.0 {

        let err = format!(
            "Invalid aperture value: {}",
            aperture
        );

        error!("{}", err);

        return Err(err);
    }

    Ok(vec![

        format!(
            "-ApertureValue={:.1}",
            aperture
        ),

        format!(
            "-FNumber={:.1}",
            aperture
        ),
    ])
}


//////////////////////////////////////////////////////
/// FOCAL LENGTH
//////////////////////////////////////////////////////

pub fn inject_focal_length_metadata_args(
    focal_length: &str,
) -> Result<Vec<String>, String> {

    let clean = focal_length
        .trim()
        .to_lowercase()
        .trim_end_matches("mm")
        .trim()
        .to_string();

    if clean.is_empty() {
        let err = "Invalid focal length".to_string();
        error!("{}", err);
        return Err(err);
    }

    // Normalize numeric values:
    // "90.0" -> "90"

    let normalized = match clean.parse::<f64>() {
        Ok(value) => {
            if value.fract() == 0.0 {
                format!("{}", value as i64)
            } else {
                value.to_string()
            }
        }
        Err(_) => clean,
    };

    Ok(vec![

        format!(
            "-FocalLength={}",
            normalized
        ),

        format!(
            "-FocalLengthIn35mmFormat={}",
            normalized
        )

    ])
}

////////////////
/// 6) Determines the orientation of an image from its Exif data.
///////////////

pub fn determine_orientation(exiftool_bin: &Path, image_path: &Path) -> Result<String, String> {
    info!("[ExifTool] Checking orientation for: {}", image_path.display());

    if !image_path.exists() {
        let err = format!("Input image file does not exist: {}", image_path.display());
        error!("{}", err);
        return Err(err);
    }

    // -S flag gives a very clean, short output (e.g., "Orientation: Horizontal (normal)")
    // -s3 prints only the actual value itself (e.g., "Horizontal (normal)")
    let output = Command::new(exiftool_bin)
        .args(["-EXIF:Orientation", "-n", "-s3", image_path.to_str().unwrap_or("")])
        .output()
        .map_err(|e| {
            let err = format!("Failed to execute ExifTool orientation check: {e}");
            error!("{}", err);
            err
        })?;
        
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err = format!("ExifTool orientation read failed: {}", stderr.trim());
        error!("{}", err);
        return Err(err);
    }

    let orientation_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if orientation_str.is_empty() {
        let warn_msg = "No orientation tag found. Defaulting to 'Unknown'.".to_string();
        warn!("[ExifTool] {} for {}", warn_msg, image_path.display());
        return Ok("Unknown".to_string());
    }

    info!("[ExifTool] Orientation for {}: {}", image_path.display(), orientation_str);
    Ok(orientation_str)
}




////////DEFINE LENS ID


pub fn lens_id_contains_known_brand(lens_id: &str) -> bool {
    KNOWN_LENS_BRANDS.iter().any(|brand| {
        let pattern = format!(r"(?i)\b{}\b", regex::escape(brand));

        Regex::new(&pattern)
            .map(|re| re.is_match(lens_id))
            .unwrap_or(false)
    })
}


////////////////////////////////////////////////////////////////////////////////
/// 9) INJECTS EXTRACTED PREVIEW BYTES INTO THE SIMPLEST ROOT LEVEL OF A DNG
/// Solución universal: Usa tags genéricos con compatibilidad de bloque DNG
////////////////////////////////////////////////////////////////////////////////
pub fn inject_extracted_preview_bytes(
    exiftool_bin: &Path,
    destination_dng: &Path,
    preview_bytes: &[u8],
) -> Result<(), String> {
    info!(
        "[ExifTool] Injecting raw preview bytes ({} bytes) universally...", 
        preview_bytes.len()
    );

    if !destination_dng.exists() {
        let err = format!("Target DNG file does not exist: {}", destination_dng.display());
        error!("{}", err);
        return Err(err);
    }

    use std::io::Write;
    use std::process::Stdio;

    // EXPLICACIÓN DE LA SOLUCIÓN NATIVA:
    // 1. Usamos "-PreviewImage<=-" para alimentar el flujo JPEG binario puro de forma genérica.
    // 2. Al NO anteponer SubIFD1:, la función es 100% segura para Sony, Nikon y Canon RAWs.
    //    ExifTool buscará automáticamente el slot de preview correcto del formato de destino.
    let mut child = Command::new(exiftool_bin)
        .args([
            "-PreviewImage<=-", 
            "-overwrite_original",
            destination_dng.to_str().unwrap_or("")
        ])
        .stdin(Stdio::piped()) 
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn ExifTool write process: {e}"))?;

    // Volcamos el vector de bytes de la imagen original directamente en la tubería
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(preview_bytes)
            .map_err(|e| format!("Failed to stream byte vector into ExifTool stdin: {e}"))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed waiting for ExifTool execution to finish: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err = format!("ExifTool preview injection failed: {}", stderr.trim());
        error!("{}", err);
        return Err(err);
    }

    info!("[ExifTool] Successfully burned full-resolution preview into correct file track layout.");
    Ok(())
}
