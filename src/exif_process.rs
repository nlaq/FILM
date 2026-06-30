use std::path::{Path, PathBuf};
use std::fs::{self};
use std::io::{Cursor};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::io::{Error, ErrorKind};
use regex::Regex;


// Link directly to your zipped resource file
const EXIFTOOL_ZIP: &[u8] = include_bytes!("../bin/exiftool.zip");

/// Safely unpacks your embedded zip to the temp directory and returns the script path
pub fn ensure_embedded_exiftool() -> Result<PathBuf, String> {
    let temp_dir = std::env::temp_dir().join("dnglab_engine_v1");
    let exiftool_executable = temp_dir.join("exiftool");

    if !exiftool_executable.exists() {
        println!("[INFO] Extracting embedded ExifTool environment to temporary directory...");
        let _ = fs::create_dir_all(&temp_dir);
        let reader = Cursor::new(EXIFTOOL_ZIP);
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let outpath = temp_dir.join(file.name());

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath).ok();
            } else {
                if let Some(p) = outpath.parent() { fs::create_dir_all(p).ok(); }
                
                let mut outfile = fs::File::create(&outpath).unwrap();
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                
                // Assign executable POSIX permissions to everyone
                let mut perms = fs::metadata(&outpath).unwrap().permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&outpath, perms);
            }
        }
        println!("[INFO] ExifTool environment successfully extracted and prepared.");
    }

    Ok(exiftool_executable)
}
//////////////////
// ── EXIF RE-INJECT TAGS INCLUDING  JPEG PREVIEW DUE TO DNGLAB AND RAWLER WRONG TAGS
//////////////////
pub fn exiftool_repair(raw_path: &Path, dng_path: &Path) -> Result<String, String> {
    let exiftool_executable = crate::exif_process::ensure_embedded_exiftool()?;
    let temp_dir = std::env::temp_dir().join("dnglab_engine_v1");

    let output = Command::new("perl")
        .env("PERL5LIB", temp_dir.join("lib"))
        .arg(&exiftool_executable)
        .arg("-tagsfromfile")
        .arg(raw_path)
        .arg("-all:all")      
        .arg("-Preview:All")  
        .arg("-overwrite_original")
        .arg(dng_path)
        .output()
        .map_err(|e| format!("ExifTool pipeline fail: {}", e))?;

    if output.status.success() {
        Ok(format!("Fixed preview metadata headers for: {:?}", dng_path.file_name().unwrap_or_default()))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}


///////////////
//////EXTRACT EXIF DATA ADN UPDATE
///////////////

#[derive(Debug, Clone, Default)]
pub struct ExifMetadata {
    pub camera: String,
    pub lens: String,
    pub aperture: String,
    pub iso: String,
    pub size: String,
}

/// Natively parses file headers to extract metadata keys.

// Define your constant list of brands outside the function for clean code structure
const KNOWN_BRANDS: &[&str] = &[
    // --- Major Modern Manufacturers ---
    "Canon", "Nikon", "Sony", "Sigma", "Tamron", "Fujifilm", "Panasonic", "Olympus", "Leica",
    "Voigtländer", "Voigtlander", "Minolta", "Pentax", "Konica", "Irix", "Lomography", "Pentacon",
    "Viltrox", "7artisans", "TTArtisan", "Laowa", "Samyang", "Rokinon", "Tokina", "Zeiss",
    "Pergear", "Zhong Yi", "Zhongyi", "Mitakon", "Brightin Star", "Sirui", "AstrHori", "Meike",
    "Thypoch", "Light Lens Lab", "Mr. Ding", "MS Optics", "MS-Optics", "Artizlab", "DJ-Optical", 
    "Omnar", "Funleader", "Kamlan", "Neewer", "Dulens",
    "Helios", "Jupiter", "Zenit", "Industar", "Mir", "Tair",
    "Meyer-Optik", "Meyer Optik", "Görlitz", "Goerlitz", "Schneider-Kreuznach", "Enna"
];
pub fn extract_important_exif(path: &Path) -> ExifMetadata {
    let mut meta = ExifMetadata {
        camera: "".to_string(),
        lens: "".to_string(),
        aperture: "".to_string(),
        iso: "".to_string(),
        size: "".to_string(),
    };

    let exiftool_executable = match ensure_embedded_exiftool() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[ERROR] Could not resolve embedded ExifTool environment: {}", e);
            return meta;
        }
    };
    
    let temp_dir = std::env::temp_dir().join("dnglab_engine_v1");
    let absolute_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    
    // ADDED: Added "-LensID" to get ExifTool's best composite lookup guess where available
    let output = Command::new("perl")
        .env("PERL5LIB", temp_dir.join("lib"))
        .arg(&exiftool_executable)
        .arg("-s")
        .arg("-Make")
        .arg("-Model")
        .arg("-LensMake")
        .arg("-LensModel")
        .arg("-LensID")
        .arg("-Aperture")
        .arg("-ISO")
        .arg("-ImageSize")
        .arg(&absolute_path)
        .output();

    match output {
        Ok(out) => {
            let stdout_str = String::from_utf8_lossy(&out.stdout);
            let mut camera_make = String::new();
            let mut camera_model = String::new();
            let mut lens_make = String::new();
            let mut lens_model = String::new();
            let mut lens_id = String::new();

            for line in stdout_str.lines() {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() < 2 { continue; }
                
                let key = parts[0].trim();
                let val = parts[1].trim().to_string();

                match key {
                    "Make"      => camera_make = val,
                    "Model"     => camera_model = val,
                    "LensMake"  => lens_make = val,
                    "LensModel" => lens_model = val,
                    "LensID"    => lens_id = val, // CAPTURED: Brand-new value target
                    "Aperture"  => meta.aperture = format!("f/{}", val.replace("f/", "")),
                    "ISO"       => meta.iso = format!("{}", val),
                    "ImageSize" => meta.size = val,
                    _ => {}
                }
            }

            // 1. UNIFICACIÓN INTELIGENTE DE CÁMARA (Marca + Modelo)
            if !camera_model.is_empty() {
                let make_lower = camera_make.to_lowercase();
                let model_lower = camera_model.to_lowercase();

                if !camera_make.is_empty() && !model_lower.starts_with(&make_lower) {
                    meta.camera = format!("{} {}", camera_make, camera_model);
                } else {
                    meta.camera = camera_model;
                }
            }

            // 2. UNIFICACIÓN INTELIGENTE DEL OBJETIVO (Hierarchical Cascade Integration)
            let (final_brand, final_model) = clean_lens_pipeline(
                &lens_id, 
                &lens_make, 
                &lens_model, 
                &camera_make
            );

            // Reconstruct the unified lens property back into your ExifMetadata struct
            if final_brand != "Unknown" && final_brand != "Unknown Lens Model" {
                meta.lens = format!("{} {}", final_brand, final_model);
            } else {
                meta.lens = final_model;
            }
        }
        Err(e) => {
            eprintln!("Command runner error spawning Perl engine instance: {:?}", e);
        }
    }

    meta
}

/// Robust regex processing pipeline to guarantee clean extraction of Lens Brand and Model
fn clean_lens_pipeline(id: &str, make: &str, model: &str, camera_make: &str) -> (String, String) {
    let id_trimmed = id.trim();
    let make_trimmed = make.trim();
    let model_trimmed = model.trim();
    let cam_make_trimmed = camera_make.trim();

    // Check if LensID has priority data (contains text letters, not just raw index numbers)
    let base_text = if !id_trimmed.is_empty() && id_trimmed.chars().any(|c| c.is_alphabetic()) {
        id_trimmed
    } else {
        model_trimmed
    };

    let combined_text = format!("{} {}", base_text, make_trimmed);
    let mut lens_brand = String::from("Unknown");

    // A. Track down the brand name using case-insensitive word boundaries
    for brand in KNOWN_BRANDS {
        let pattern = format!(r"(?i)\b{}\b", regex::escape(brand));
        if let Ok(re) = Regex::new(&pattern) {
            if re.is_match(&combined_text) {
                lens_brand = brand.to_string();
                break;
            }
        }
    }

    // B. Apply Fallback Hierarchy if no popular third-party brand matched
    if lens_brand == "Unknown" {
        if !make_trimmed.is_empty() {
            lens_brand = make_trimmed.to_string();
        } else if !cam_make_trimmed.is_empty() {
            lens_brand = cam_make_trimmed.to_string();
        }
    }

    // C. Sanitize and clean up the Lens Model text expression
    let mut cleaned_model = base_text.to_string();
    if cleaned_model.is_empty() && !make_trimmed.is_empty() {
        cleaned_model = make_trimmed.to_string();
    }

    if lens_brand != "Unknown" {
        // Strip out redundant brand names if they match at the beginning
        let prefix_pattern = format!(r"(?i)^{}\s*", regex::escape(&lens_brand));
        if let Ok(re) = Regex::new(&prefix_pattern) {
            cleaned_model = re.replace(&cleaned_model, "").into_owned();
        }
    }

    // Collapse trailing or internal structural multi-spaces
    if let Ok(re_spaces) = Regex::new(r"\s+") {
        cleaned_model = re_spaces.replace_all(&cleaned_model, " ").into_owned();
    }

    let final_model = if cleaned_model.trim().is_empty() {
        String::from("Unknown Lens Model")
    } else {
        cleaned_model.trim().to_string()
    };

    (lens_brand, final_model)
}


/////////////////
////////// WRITE APERTURE AND LENS EXIF DATA 
////////////////

pub fn write_lens_and_aperture_metadata(
    file_path: &Path, 
    brand: &str,
    model: &str,
    focal_length: &str,     
    max_aperture: &str,     
    aperture_value: &str,   
) -> Result<String, std::io::Error> {
    
    if !file_path.exists() {
        return Err(Error::new(ErrorKind::NotFound, "Target RAW file not found"));
    }

    let exiftool_executable = match ensure_embedded_exiftool() {
        Ok(p) => p,
        Err(e) => return Err(Error::new(ErrorKind::Other, e)),
    };
    
    let temp_dir = std::env::temp_dir().join("dnglab_engine_v1");

    // ── 1. Enforce Explicit Layout Formatting ───────────────────────────────
    
    // 1. Ensure focal length has its "mm" unit appended cleanly
    let focal_with_unit = if focal_length.ends_with("mm") {
        focal_length.to_string()
    } else {
        format!("{}mm", focal_length)
    };

    // 2. Ensure maximum aperture has its "f/" prefix stripped for numbers, 
    // but we'll use it elegantly in the descriptive text string.
    let clean_max_aperture = max_aperture
        .to_lowercase()
        .replace("f/", "")
        .trim()
        .to_string();

    let brand_lower = brand.to_lowercase().trim().to_string();
    let model_lower = model.to_lowercase().trim().to_string();
    
    // 3. Extract the base model name while preventing brand duplication
    let base_model = if model_lower.starts_with(&brand_lower) {
        // Strip the brand from the front if it's there, we'll format it cleanly next
        model[brand.len()..].trim().to_string()
    } else {
        model.trim().to_string()
    };

    // 4. CRITICAL FIX: Explicitly construct the exact requested target string layout:
    // Layout format result: "Brand Model FocalLength f/MaxAperture" 
    // Example output: "Leica Summicron 50mm f/2"
    let target_lens_model = if brand_lower == "unknown" || brand_lower.is_empty() {
        format!("{} {} f/{}", base_model, focal_with_unit, clean_max_aperture)
    } else {
        format!("{} {} {} f/{}", brand.trim(), base_model, focal_with_unit, clean_max_aperture)
    };

    // Prepare standard EXIF array layout boundary [MinFocal MaxFocal MaxAp MaxAp]
    let focal_clean = focal_with_unit.trim_end_matches("mm").trim();
    let focal_bounds: Vec<&str> = focal_clean.split('-').collect();
    let min_focal = focal_bounds.first().copied().unwrap_or("0");
    let max_focal = focal_bounds.last().copied().unwrap_or(min_focal);
    let target_lens_info = format!("{} {} {} {}", min_focal, max_focal, clean_max_aperture, clean_max_aperture);

    let cleaned_aperture = aperture_value
        .to_lowercase()
        .replace("f/", "")
        .trim()
        .to_string();

    // ── 2. Build and Pipe the Exiftool Process ────────────────────────────
    println!("\n--- [EXIFTOOL RUN] Processing: {:?} ---", file_path.file_name().unwrap_or_default());

    let mut cmd = Command::new("perl");
    
    cmd.env("PERL5LIB", temp_dir.join("lib"))
       .arg(&exiftool_executable)
       .arg("-overwrite_original")
       
       // Step A: Blindly force-wipe the Sony MakerNotes identifiers using raw byte values (#)
       //.arg("-LensType#=0")
       //.arg("-LensType2#=0")
       //.arg("-LensType3#=0")
       //.arg("-LensSpec=")
       
       // Step B: Overwrite the universal string components
       .arg(format!("-LensMake={}", brand.trim()))
       .arg(format!("-LensModel={}", target_lens_model))
       .arg(format!("-LensInfo={}", target_lens_info));

    if !cleaned_aperture.is_empty() && cleaned_aperture != "select..." {
        cmd.arg(format!("-EXIF:FNumber={}", cleaned_aperture))
           .arg(format!("-EXIF:ApertureValue={}", cleaned_aperture));
    }

    cmd.arg(file_path);

    let status = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?; 

    println!("---------------------------------------------------\n");
    
    if status.success() {
        Ok(format!("[SUCCESS] Completed execution for {:?}", file_path.file_name().unwrap_or_default()))
    } else {
        Err(Error::new(
            ErrorKind::Other, 
            format!("ExifTool exited with non-zero error code: {}", status.code().unwrap_or(-1))
        ))
    }
}