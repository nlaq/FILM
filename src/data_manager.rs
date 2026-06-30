use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Lens {
    pub brand: String,
    pub model: String,
    pub focal: String,
    pub max_aperture: String,
    pub min_aperture: String,
    
    // This field exists purely in memory. It won't read or write to JSON!
    #[serde(skip)]
    pub id: String, 
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Brand {
    pub brand_name: String,
    pub models: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LensData {
    pub lenses: Vec<Lens>,
    pub brands: Vec<Brand>,
    pub focal_lengths: Vec<String>,
    pub apertures: Vec<String>,
}

pub struct DataManager {
    file_path: String,
    pub data: LensData,
}

// ── Sorting helpers ────────────────────────────────────────────────────────────

/// "135mm" → 135.0
fn focal_to_f64(s: &str) -> f64 {
    s.chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .parse::<f64>()
        .unwrap_or(f64::MAX)
}

/// "2.8" → 2.8  (no f/ prefix in data anymore)
fn aperture_to_f64(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(f64::MAX)
}

impl DataManager {

        // Sorts lenses first by brand, then by model, then by focal length numerically
        fn sort_all(data: &mut LensData) {
        // 1. Run your original sorting logic first
        data.lenses.sort_by(|a, b| {
            let brand_cmp = a.brand.to_lowercase().cmp(&b.brand.to_lowercase());
            if brand_cmp != std::cmp::Ordering::Equal { return brand_cmp; }
            let model_cmp = a.model.to_lowercase().cmp(&b.model.to_lowercase());
            if model_cmp != std::cmp::Ordering::Equal { return model_cmp; }
            focal_to_f64(&a.focal).partial_cmp(&focal_to_f64(&b.focal)).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        for (idx, lens) in data.lenses.iter_mut().enumerate() {
            lens.id = format!("{}_{}_{}_{}", lens.brand, lens.model, lens.focal, idx);
        }
        
        data.brands.sort_by(|a, b| {
            a.brand_name.to_lowercase().cmp(&b.brand_name.to_lowercase())
        });
        for brand in data.brands.iter_mut() {
            brand.models.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        }

        data.focal_lengths.sort_by(|a, b| {
            focal_to_f64(a).partial_cmp(&focal_to_f64(b)).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Ascending by f-number: 1.4, 1.8, 2, 2.8 … 22, 32
        data.apertures.sort_by(|a, b| {
            aperture_to_f64(a).partial_cmp(&aperture_to_f64(b)).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Loads JSON into memory and sorts all lists.
    pub fn new(file_path: &str) -> Result<Self, String> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(format!("Data file not found at: {}", file_path));
        }

        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(|e| e.to_string())?;

        let mut data: LensData = serde_json::from_str(&contents)
            .map_err(|e| format!("JSON parsing failed: {}", e))?;

        Self::sort_all(&mut data);

        Ok(DataManager {
            file_path: file_path.to_string(),
            data,
        })
    }

    pub fn save(&self) -> Result<(), String> {
        let serialized = serde_json::to_string_pretty(&self.data)
            .map_err(|e| e.to_string())?;
        let mut file = File::create(&self.file_path).map_err(|e| e.to_string())?;
        file.write_all(serialized.as_bytes()).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Adds a lens using the explicit field structure.
    pub fn add_lens(
        &mut self, 
        brand: String, 
        model: String, 
        focal: String, 
        max_ap: String, 
        min_ap: String
    ) -> Result<(), String> {
        // Prevent exact duplicates based on brand, model, and focal length
        let exists = self.data.lenses.iter().any(|l| {
            l.brand.to_lowercase() == brand.to_lowercase() 
                && l.model.to_lowercase() == model.to_lowercase()
                && l.focal.to_lowercase() == focal.to_lowercase()
        });

        if exists {
            return Ok(());
        }

        self.data.lenses.push(Lens {
            brand,
            model,
            focal,
            max_aperture: max_ap,
            min_aperture: min_ap,
            id: String::new(), // ← Add this line to initialize the temporary blank ID
        });

        // Re-run sort_all to keep everything aligned and generate the true tracking ID
        Self::sort_all(&mut self.data);
        self.save()
    }

    pub fn add_brand(&mut self, brand_name: String) -> Result<(), String> {
        let exists = self.data.brands.iter().any(|b| {
            b.brand_name.to_lowercase() == brand_name.to_lowercase()
        });
        if !exists {
            self.data.brands.push(Brand { brand_name, models: Vec::new() });
            self.data.brands.sort_by(|a, b| {
                a.brand_name.to_lowercase().cmp(&b.brand_name.to_lowercase())
            });
            self.save()?;
        }
        Ok(())
    }

    pub fn add_model_to_brand(&mut self, brand_name: &str, model_name: String) -> Result<(), String> {
        if let Some(brand) = self.data.brands.iter_mut().find(|b| b.brand_name == brand_name) {
            let exists = brand.models.iter().any(|m| m.to_lowercase() == model_name.to_lowercase());
            if !exists {
                brand.models.push(model_name);
                brand.models.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
                self.save()?;
            }
            Ok(())
        } else {
            Err(format!("Brand '{}' not found in database.", brand_name))
        }
    }

    /// `focal` should be the bare number + unit, e.g. "85mm".
    pub fn add_focal_length(&mut self, focal: String) -> Result<(), String> {
        let exists = self.data.focal_lengths.iter().any(|f| f.to_lowercase() == focal.to_lowercase());
        if !exists {
            self.data.focal_lengths.push(focal);
            self.data.focal_lengths.sort_by(|a, b| {
                focal_to_f64(a).partial_cmp(&focal_to_f64(b)).unwrap_or(std::cmp::Ordering::Equal)
            });
            self.save()?;
        }
        Ok(())
    }

    /// `aperture` is a bare number e.g. "1.4", "2.8", "22".
    pub fn add_aperture(&mut self, aperture: String) -> Result<(), String> {
        let exists = self.data.apertures.iter().any(|a| a == &aperture);
        if !exists {
            self.data.apertures.push(aperture);
            self.data.apertures.sort_by(|a, b| {
                aperture_to_f64(a).partial_cmp(&aperture_to_f64(b)).unwrap_or(std::cmp::Ordering::Equal)
            });
            self.save()?;
        }
        Ok(())
    }

    // ── Remove methods ─────────────────────────────────────────────────────────

    /// Removes a lens matching brand, model, and focal length.
    pub fn remove_lens(&mut self, brand: &str, model: &str, focal: &str) -> Result<(), String> {
        let before = self.data.lenses.len();
        self.data.lenses.retain(|l| {
            !(l.brand.to_lowercase() == brand.to_lowercase()
                && l.model.to_lowercase() == model.to_lowercase()
                && l.focal.to_lowercase() == focal.to_lowercase())
        });
        if self.data.lenses.len() < before {
            self.save()?;
        }
        Ok(())
    }

    /// Removes a brand (and all its models) by exact name and saves.
    pub fn remove_brand(&mut self, brand_name: &str) -> Result<(), String> {
        let before = self.data.brands.len();
        self.data.brands.retain(|b| b.brand_name != brand_name);
        if self.data.brands.len() < before {
            self.save()?;
        }
        Ok(())
    }

    /// Removes a model from a specific brand and saves.
    pub fn remove_model_from_brand(&mut self, brand_name: &str, model_name: &str) -> Result<(), String> {
        if let Some(brand) = self.data.brands.iter_mut().find(|b| b.brand_name == brand_name) {
            let before = brand.models.len();
            brand.models.retain(|m| m != model_name);
            if brand.models.len() < before {
                self.save()?;
            }
            Ok(())
        } else {
            Err(format!("Brand '{}' not found.", brand_name))
        }
    }

    /// Removes a focal length by exact string e.g. "85mm" and saves.
    pub fn remove_focal_length(&mut self, focal: &str) -> Result<(), String> {
        let before = self.data.focal_lengths.len();
        self.data.focal_lengths.retain(|f| f != focal);
        if self.data.focal_lengths.len() < before {
            self.save()?;
        }
        Ok(())
    }

    /// Removes a bare aperture value e.g. "2.8" and saves.
    pub fn remove_aperture(&mut self, aperture: &str) -> Result<(), String> {
        let before = self.data.apertures.len();
        self.data.apertures.retain(|a| a != aperture);
        if self.data.apertures.len() < before {
            self.save()?;
        }
        Ok(())
    }
    
    
    pub fn get_lens_by_id(&self, id: &str) -> Option<&Lens> {
        self.data.lenses.iter().find(|l| l.id == id)
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Settings
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    #[serde(default)]
    pub artist:            String,
    #[serde(default = "Settings::default_compression")]
    pub compression:       String,
    #[serde(default = "Settings::default_crop")]
    pub crop:              String,
    #[serde(default = "Settings::default_true")]
    pub dng_preview:       bool,
    #[serde(default = "Settings::default_true")]
    pub dng_thumbnail:     bool,
    #[serde(default = "Settings::default_true")]
    pub embed_raw:         bool,
    #[serde(default)]
    pub override_files:    bool,
    #[serde(default = "Settings::default_index")]
    pub image_index:       String,
    #[serde(default = "Settings::default_predictor")]
    pub ljpeg92_predictor: u8,
}

impl Settings {
    fn default_compression() -> String { "lossless".into() }
    fn default_crop()        -> String { "best".into() }
    fn default_index()       -> String { "0".into() }
    fn default_true()        -> bool   { true }
    fn default_predictor()   -> u8     { 1 }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            artist:            String::new(),
            compression:       Self::default_compression(),
            crop:              Self::default_crop(),
            dng_preview:       true,
            dng_thumbnail:     true,
            embed_raw:         true,
            override_files:    false,
            image_index:       Self::default_index(),
            ljpeg92_predictor: 1,
        }
    }
}

pub struct SettingsManager {
    file_path: String,
    pub data: Settings,
}

impl SettingsManager {
    /// Loads settings.json from `file_path`. If the file does not exist,
    /// creates it with defaults. Corrupt JSON also falls back to defaults.
    pub fn new(file_path: &str) -> Result<Self, String> {
        let path = Path::new(file_path);

        let data = if path.exists() {
            let mut file = File::open(path).map_err(|e| e.to_string())?;
            let mut contents = String::new();
            file.read_to_string(&mut contents).map_err(|e| e.to_string())?;
            serde_json::from_str::<Settings>(&contents).unwrap_or_default()
        } else {
            // First launch — write default file so it's visible to the user
            let defaults = Settings::default();
            let json = serde_json::to_string_pretty(&defaults)
                .map_err(|e| e.to_string())?;
            let mut file = File::create(path).map_err(|e| e.to_string())?;
            file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
            defaults
        };

        Ok(SettingsManager {
            file_path: file_path.to_string(),
            data,
        })
    }

    /// Serializes current data back to settings.json.
    pub fn save(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.data)
            .map_err(|e| e.to_string())?;
        let mut file = File::create(&self.file_path).map_err(|e| e.to_string())?;
        file.write_all(json.as_bytes()).map_err(|e| e.to_string())?;
        Ok(())
    }

    // ── Setters (mutate + auto-save, same pattern as DataManager) ────────────

    pub fn set_artist(&mut self, v: String) -> Result<(), String> {
        self.data.artist = v; self.save()
    }
    pub fn set_compression(&mut self, v: String) -> Result<(), String> {
        self.data.compression = v; self.save()
    }
    pub fn set_crop(&mut self, v: String) -> Result<(), String> {
        self.data.crop = v; self.save()
    }
    pub fn set_dng_preview(&mut self, v: bool) -> Result<(), String> {
        self.data.dng_preview = v; self.save()
    }
    pub fn set_dng_thumbnail(&mut self, v: bool) -> Result<(), String> {
        self.data.dng_thumbnail = v; self.save()
    }
    pub fn set_embed_raw(&mut self, v: bool) -> Result<(), String> {
        self.data.embed_raw = v; self.save()
    }
    pub fn set_override_files(&mut self, v: bool) -> Result<(), String> {
        self.data.override_files = v; self.save()
    }
    pub fn set_image_index(&mut self, v: String) -> Result<(), String> {
        self.data.image_index = v; self.save()
    }
    pub fn set_ljpeg92_predictor(&mut self, v: u8) -> Result<(), String> {
        self.data.ljpeg92_predictor = v.clamp(1, 7); self.save()
    }
}