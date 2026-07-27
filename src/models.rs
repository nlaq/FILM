use serde::{Deserialize, Serialize};
use std::path::PathBuf;



#[derive(Debug, Clone)]
pub struct ConversionRequest {
    /// RAW files to convert
    pub input_files: Vec<PathBuf>,

    /// Destination directory for generated DNGs
    pub output_directory: PathBuf,
    
    /// "same" or "pick"
    //pub output_mode: String,
        
    // ----- DNG settings -----

    pub artist: String,
    pub compression: String,
    pub crop: String,

    pub dng_preview: bool,
    pub dng_thumbnail: bool,
    pub embed_raw: bool,
    pub override_files: bool,

    pub image_index: String,
    pub ljpeg92_predictor: u8,
}

// ==========================================
// LENS EXIF DATA MODELS
// ==========================================

#[derive(Debug, Deserialize, Clone)]
pub struct CameraMetadata {
    #[serde(alias = "Model")]
    pub camera_model: Option<String>,
    #[serde(alias = "Make")]
    pub camera_make: Option<String>,
    #[serde(alias = "LensModel")]
    pub lens_model: Option<String>,
    #[serde(alias = "LensMake")]
    pub lens_make: Option<String>,
    #[serde(alias = "LensID")]         
    pub lens_id: Option<String>,      
    #[serde(alias = "Aperture")]
    pub aperture: Option<f64>,
    #[serde(alias = "ISO")]
    pub iso: Option<u32>,
    #[serde(alias = "ImageSize")]      
    pub image_size: Option<String>,    
}
// ==========================================
// DATA.JSON MODELS
// ==========================================

/// Represents an individual lens item with its technical specs
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LensItem {
    pub id: u64,
    pub brand: String,
    pub model: String,
    pub focal: String,
    pub max_aperture: String,
    pub min_aperture: String,
}

/// Represents a brand grouping and its associated camera models
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BrandItem {
    pub brand_name: String,
    pub models: Vec<String>,
}

/// The top-level mapping structure matching your exact data.json layout
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LensDatabase {
    pub lenses: Vec<LensItem>,
    pub brands: Vec<BrandItem>,
    pub focal_lengths: Vec<String>,
    pub apertures: Vec<String>,
}

// ==========================================
// SETTINGS.JSON MODELS
// ==========================================

/// Represents the application configurations layout matching settings.json
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub artist: String,
    pub compression: String,
    pub crop: String,
    pub dng_preview: bool,
    pub dng_thumbnail: bool,
    pub embed_raw: bool,
    pub override_files: bool,
    pub image_index: String, // Kept as a String to match the "0" in your JSON
    pub ljpeg92_predictor: u8,
}


