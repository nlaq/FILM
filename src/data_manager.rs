use std::path::{Path, PathBuf};
use log::{info, error, warn};
use crate::models::{LensDatabase, LensItem, AppSettings};
use serde::{Serialize, Deserialize};


/////////////////
/////LENSES DATA
////////////////


pub struct DataManager {
    file_path: PathBuf,
    pub database: LensDatabase,
}

impl DataManager {
    /// Initializes the DataManager by loading and validating the complete JSON file into RAM.
    /// Captures the file path internally for future write operations.
    pub fn new(file_path: &Path) -> Result<Self, String> {
        info!("[DataManager] Attempting to load the lens database from: {}", file_path.display());
        
        // Replace LensDatabase::load_from_file(file_path) with this:
let file_content = std::fs::read_to_string(file_path).map_err(|e| e.to_string())?;
let database: LensDatabase = serde_json::from_str(&file_content).map_err(|e| e.to_string())?;

        info!("[DataManager] Lens database successfully loaded into memory.");
        Ok(Self { 
            database,
            file_path: file_path.to_path_buf(),
        })
    }
    
    
    

    //////////////////////
    // READ FUNCTIONS
    //////////////////////

    /// 1) Reads all lenses along with all their inner attributes (brand, model, focal, max_aperture, min_aperture)
    /// and returns them sorted alphabetically by brand name, then by model name.
    pub fn get_sorted_lenses(&self) -> Vec<LensItem> {
        info!("[DataManager] Reading all detailed lenses (brand, model, focal, max/min apertures)...");
        let mut lenses = self.database.lenses.clone();

        lenses.sort_by(|a, b| {
            a.brand.to_lowercase().cmp(&b.brand.to_lowercase())
                .then_with(|| a.model.to_lowercase().cmp(&b.model.to_lowercase()))
        });

        info!("[DataManager] {} detailed lenses successfully read and sorted.", lenses.len());
        lenses
    }

    /// 2) Reads all available aperture values from the master apertures list 
    /// and returns them in true numerical order (from smallest value to higher).
    pub fn get_sorted_apertures(&self) -> Vec<String> {
        info!("[DataManager] Reading master aperture constraints...");
        let mut apertures = self.database.apertures.clone();

        apertures.sort_by(|a, b| {
            let a_num = a.parse::<f64>().unwrap_or(0.0);
            let b_num = b.parse::<f64>().unwrap_or(0.0);
            a_num.partial_cmp(&b_num).unwrap_or(std::cmp::Ordering::Equal)
        });

        info!("[DataManager] {} master apertures read and numerically ordered.", apertures.len());
        apertures
    }

    /// 3) Reads all available focal lengths from the master focal_lengths list 
    /// and returns them in true numerical order (from smallest value to higher).
    pub fn get_sorted_focal_lengths(&self) -> Vec<String> {
        info!("[DataManager] Reading master focal length constraints...");
        let mut focal_lengths = self.database.focal_lengths.clone();

        focal_lengths.sort_by(|a, b| {
            let a_num = a.parse::<f64>().unwrap_or(0.0);
            let b_num = b.parse::<f64>().unwrap_or(0.0);
            a_num.partial_cmp(&b_num).unwrap_or(std::cmp::Ordering::Equal)
        });

        info!("[DataManager] {} master focal lengths read and numerically ordered.", focal_lengths.len());
        focal_lengths
    }

    /// 4) Reads all registered brand groupings from the brands array list 
    /// and returns just their brand names sorted alphabetically.
    pub fn get_sorted_brands(&self) -> Vec<String> {
        info!("[DataManager] Reading all unique lens brand profiles...");
        let mut brands: Vec<String> = self.database.brands.iter().map(|b| b.brand_name.clone()).collect();
        
        brands.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        info!("[DataManager] {} brand profiles successfully processed.", brands.len());
        brands
    }

    /// 5) Given a specific brand string, filters out and reads all associated sub-models 
    /// explicitly belonging to that target manufacturer, returned in alphabetical order.
    pub fn get_sorted_models_for_brand(&self, target_brand: &str) -> Result<Vec<String>, String> {
        info!("[DataManager] Querying model sub-registry for brand: '{}'", target_brand);

        let brand_entry = self.database.brands.iter()
            .find(|b| b.brand_name.to_lowercase() == target_brand.to_lowercase());

        match brand_entry {
            Some(item) => {
                let mut models = item.models.clone();
                models.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
                
                info!("[DataManager] Found and sorted {} models under brand profile '{}'.", models.len(), target_brand);
                Ok(models)
            }
            None => {
                let err_msg = format!("Brand lookup failed: '{}' does not exist in data profiles.", target_brand);
                warn!("[DataManager] {}", err_msg);
                Err(err_msg)
            }
        }
    }
    
    /////////////////
    //////Read the aperture range for a specific lens.
    
    pub fn get_apertures_for_lens(
        &self,
        lens: &LensItem,
    ) -> Vec<String> {
    
        let min = lens
            .min_aperture
            .parse::<f64>()
            .unwrap_or(0.0);
    
        let max = lens
            .max_aperture
            .parse::<f64>()
            .unwrap_or(0.0);
    
    
        self.get_sorted_apertures()
            .into_iter()
            .filter(|a| {
    
                let value =
                    a.parse::<f64>()
                    .unwrap_or(0.0);
    
                value >= max && value <= min
    
            })
            .collect()
    }

    //////////////////////
    // WRITE FUNCTIONS
    //////////////////////

    /// Helper function to persist the current in-memory database state back to the JSON file.
    /// Uses the internal file_path saved at initialization.
    fn save_to_disk(&self) -> Result<(), String> {
        info!("[DataManager] Saving updated database state to disk: {}", self.file_path.display());
        
        let file = std::fs::File::create(&self.file_path).map_err(|e| {
            let err = format!("Failed to create/open database file for writing: {e}");
            error!("{}", err);
            err
        })?;
        
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self.database).map_err(|e| {
            let err = format!("Failed to serialize database to JSON format: {e}");
            error!("{}", err);
            err
        })?;

        info!("[DataManager] Database successfully serialized and saved to disk.");
        Ok(())
    }

    /// 1) Adds a new brand to the brands master registry.
    /// Returns an error if the brand already exists.
    pub fn add_brand(&mut self, brand_name: &str) -> Result<(), String> {
        info!("[DataManager] Attempting to add brand: '{}'", brand_name);

        let brand_exists = self.database.brands.iter()
            .any(|b| b.brand_name.to_lowercase() == brand_name.to_lowercase());

        if brand_exists {
            let err = format!("Validation Error: Brand '{}' already exists.", brand_name);
            warn!("{}", err);
            return Err(err);
        }

        self.database.brands.push(crate::models::BrandItem {
            brand_name: brand_name.to_string(),
            models: Vec::new(),
        });

        info!("[DataManager] Brand '{}' added to memory profile.", brand_name);
        self.save_to_disk()
    }

    /// 2) Adds a new model name under an existing brand's sub-registry.
    /// Enforces that the parent brand MUST already exist in the database profile.
    pub fn add_model(&mut self, brand_name: &str, model_name: &str) -> Result<(), String> {
        info!("[DataManager] Attempting to append model '{}' into existent brand context '{}'", model_name, brand_name);

        if model_name.trim().is_empty() {
            let err = "Validation Error: Model name cannot be blank.".to_string();
            error!("{}", err);
            return Err(err);
        }

        // Locate the existing brand mutably. If it doesn't exist, reject the operation immediately.
        let brand_entry = self.database.brands.iter_mut()
            .find(|b| b.brand_name.to_lowercase() == brand_name.to_lowercase());

        match brand_entry {
            Some(brand_item) => {
                let model_exists = brand_item.models.iter()
                    .any(|m| m.to_lowercase() == model_name.to_lowercase());

                if model_exists {
                    let err = format!(
                        "Validation Error: Model '{}' is already registered under the brand '{}'.", 
                        model_name, brand_item.brand_name
                    );
                    warn!("{}", err);
                    return Err(err);
                }

                brand_item.models.push(model_name.to_string());
                info!(
                    "[DataManager] Model '{}' successfully linked to existing brand '{}' in memory.", 
                    model_name, brand_item.brand_name
                );

                self.save_to_disk()
            }
            None => {
                let err = format!(
                    "Structural Constraint Failure: Cannot add model '{}'. The brand '{}' does not exist in the database. Please create the brand first.", 
                    model_name, brand_name
                );
                error!("{}", err);
                Err(err)
            }
        }
    }

    /// 3) Adds a new focal length value string to the master focal_lengths array list.
    /// Returns an error if the focal length value already exists.
    pub fn add_focal_length(&mut self, focal_length: &str) -> Result<(), String> {
        info!("[DataManager] Attempting to add focal length configuration: '{}'", focal_length);

        if focal_length.trim().is_empty() {
            let err = "Validation Error: Focal length string value cannot be empty.".to_string();
            error!("{}", err);
            return Err(err);
        }

        let exists = self.database.focal_lengths.iter().any(|f| f == focal_length);
        if exists {
            let err = format!("Validation Error: Focal length '{}' already exists in master constraints.", focal_length);
            warn!("{}", err);
            return Err(err);
        }

        self.database.focal_lengths.push(focal_length.to_string());
        info!("[DataManager] Focal length '{}' added to master list configuration.", focal_length);
        self.save_to_disk()
    }

    /// 4) Adds a new aperture value string to the master apertures array list.
    /// Returns an error if the aperture value already exists.
    pub fn add_aperture(&mut self, aperture: &str) -> Result<(), String> {
        info!("[DataManager] Attempting to add aperture configuration: '{}'", aperture);

        if aperture.trim().is_empty() {
            let err = "Validation Error: Aperture string value cannot be empty.".to_string();
            error!("{}", err);
            return Err(err);
        }

        let exists = self.database.apertures.iter().any(|a| a == aperture);
        if exists {
            let err = format!("Validation Error: Aperture '{}' already exists in master constraints.", aperture);
            warn!("{}", err);
            return Err(err);
        }

        self.database.apertures.push(aperture.to_string());
        info!("[DataManager] Aperture '{}' added to master list configuration.", aperture);
        self.save_to_disk()
        
    }

    /// 5) Adds a complete LensItem structure into the lenses array database.
    /// Returns an error if an identical lens match (same brand and exact model name) already exists.
    pub fn add_lens(&mut self, mut new_lens: LensItem) -> Result<(), String> {
        info!("[DataManager] Attempting to register complex lens item: {} - {}", new_lens.brand, new_lens.model);

        let lens_exists = self.database.lenses.iter().any(|l| {
            l.brand.eq_ignore_ascii_case(&new_lens.brand)
                && l.model.eq_ignore_ascii_case(&new_lens.model)
                && l.focal == new_lens.focal
                && l.max_aperture == new_lens.max_aperture
        });

        if lens_exists {
            let err = format!(
                "Validation Error: Lens '{} {} {}mm f/{}' is already registered.",
                new_lens.brand,
                new_lens.model,
                new_lens.focal,
                new_lens.max_aperture,
            );
        
            warn!("{}", err);
            return Err(err);
        }
        new_lens.id = self.database.lenses
            .iter()
            .map(|l| l.id)
            .max()
            .unwrap_or(0) + 1;
        self.database.lenses.push(new_lens);
        info!("[DataManager] New lens item profile successfully added to memory list.");
        self.save_to_disk()
        
    }
    
    /////////////////////////
    /////DELETE FUNTIONS
    ////////////////////////
    
    // Add these functions inside the `impl DataManager` block in data_manager.rs

    /// Removes a specific lens matching both brand and model, then updates disk.
    pub fn remove_lens(&mut self, id: u64) -> Result<(), String> {
        info!("[DataManager] Attempting to delete lens with id: {}", id);
    
        let initial_len = self.database.lenses.len();
    
        self.database.lenses.retain(|l| l.id != id);
    
        if self.database.lenses.len() == initial_len {
            return Err("Target lens profile not found in database.".to_string());
        }
    
        info!("[DataManager] Lens profile successfully removed.");
    
        self.save_to_disk()
        
    }

    /// Removes a brand and all nested sub-models. 
    /// Also purges any individual complex lenses belonging to this brand to prevent orphan entries.
    pub fn remove_brand(&mut self, brand_name: &str) -> Result<(), String> {
        info!("[DataManager] Purging brand registry profile and cascading dependencies for: '{}'", brand_name);
        
        let target = brand_name.to_lowercase();
        self.database.brands.retain(|b| b.brand_name.to_lowercase() != target);
        self.database.lenses.retain(|l| l.brand.to_lowercase() != target);

        info!("[DataManager] Brand profile and cascading dependencies cleared from memory.");
        self.save_to_disk()
    }

    /// Removes a specific model sub-entry under a designated parent brand context.
    pub fn remove_model(&mut self, brand_name: &str, model_name: &str) -> Result<(), String> {
        info!("[DataManager] Removing model registration '{}' from brand context '{}'", model_name, brand_name);
        
        if let Some(brand_item) = self.database.brands.iter_mut().find(|b| b.brand_name.to_lowercase() == brand_name.to_lowercase()) {
            brand_item.models.retain(|m| m.to_lowercase() != model_name.to_lowercase());
        }
        
        // Also clean up any complex built lenses matching this layout definition
        self.database.lenses.retain(|l| {
            !(l.brand.to_lowercase() == brand_name.to_lowercase() && l.model.to_lowercase() == model_name.to_lowercase())
        });

        info!("[DataManager] Model variant safely removed.");
        self.save_to_disk()
    }

    /// Removes a focal length constraint from the master array.
    pub fn remove_focal_length(&mut self, focal_length: &str) -> Result<(), String> {
        info!("[DataManager] Removing master focal length constraint reference: {}", focal_length);
        self.database.focal_lengths.retain(|f| f != focal_length);
        self.save_to_disk()
    }

    /// Removes an aperture configuration constraint from the master array.
    pub fn remove_aperture(&mut self, aperture: &str) -> Result<(), String> {
        info!("[DataManager] Removing master aperture constraint reference: {}", aperture);
        self.database.apertures.retain(|a| a != aperture);
        self.save_to_disk()
        
    }
    
    
    
    
}





//////////////
/////SETTINGS DATA
//////////////


#[derive(Debug, Serialize, Deserialize, Clone)]



pub struct SettingsManager {
    pub settings: AppSettings,
    file_path: PathBuf,
}

impl SettingsManager {
    /// 1) Reads all settings and initializes the SettingsManager structure cache.
    pub fn new(file_path: &Path) -> Result<Self, String> {
        info!("[SettingsManager] Initializing settings from: {}", file_path.display());

        if !file_path.exists() {
            let err = format!("Settings file not found: {}", file_path.display());
            error!("{}", err);
            return Err(err);
        }

        let file = std::fs::File::open(file_path).map_err(|e| format!("Failed to open settings: {e}"))?;
        let reader = std::io::BufReader::new(file);
        let settings: AppSettings = serde_json::from_reader(reader)
            .map_err(|e| format!("Failed to parse settings JSON: {e}"))?;

        info!("[SettingsManager] Settings loaded successfully into RAM cache.");
        Ok(Self {
            settings,
            file_path: file_path.to_path_buf(),
        })
    }

    /// Internal helper method to write memory updates down to disk.
    pub fn save_to_disk(&self) -> Result<(), String> {
        info!("[SettingsManager] Persisting modified configurations to disk...");
        let file = std::fs::File::create(&self.file_path).map_err(|e| format!("Disk access error: {e}"))?;
        let writer = std::io::BufWriter::new(file);
        
        serde_json::to_writer_pretty(writer, &self.settings)
            .map_err(|e| format!("Failed to serialize settings structure: {e}"))?;

        info!("[SettingsManager] Settings successfully saved.");
        Ok(())
    }

    /// Exposes a copy of the loaded configurations.
    pub fn get_settings(&self) -> AppSettings {
        self.settings.clone()
    }

    }