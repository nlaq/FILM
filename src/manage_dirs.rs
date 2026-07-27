use std::path::{PathBuf};
use std::sync::Arc;
use log::error;
use std::cell::RefCell;
use std::rc::Rc;
use crate::DataManager;
use crate::SettingsManager;

///////////////////////
//////// PATH FOR CACHE
//////////////////////



/// Resolves the OS-specific cache directory path.
pub fn get_cache_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cache_path = match dirs::cache_dir() {
        Some(path) => path.join("film"),
        None => std::env::temp_dir().join("film"),
    };
    Ok(cache_path)
}

/// Initializes the cache system architecture layout and returns a thread-safe path pointer.
pub fn initialize_cache() -> Arc<PathBuf> {
    let cache_dir = match get_cache_path() {
        Ok(path) => path,
        Err(e) => {
            error!("[CRITICAL] Failed to resolve cache directory path context: {}", e);
            panic!("Workspace initialization path failure: {}", e);
        }
    };

    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        error!("[CRITICAL] Failed to construct core cache architecture layout on disk: {}", e);
        panic!("Workspace initialization filesystem write failure: {}", e);
    }

    Arc::new(cache_dir)
}

//////////////////
//////CLEAN CACHE
/////////////////
pub fn clean_cache() -> Result<(), Box<dyn std::error::Error>> {
    let cache_path = get_cache_path()?;
    if cache_path.exists() {
        for entrada in std::fs::read_dir(&cache_path)? {
            let entrada = entrada?;
            let path = entrada.path();
            if path.is_file() { 
                let _ = std::fs::remove_file(path); 
            }
        }
        println!("[INFO] Cache folder cleared successfully.");
    }
    Ok(())
}

/*/ Resolves data paths and initializes both Data and Settings managers wrapped in thread-safe containers.
pub fn initialize_managers() -> (Rc<RefCell<DataManager>>, Rc<RefCell<SettingsManager>>) {
    let base_dir = env!("CARGO_MANIFEST_DIR");
    
    // 1. Resolve absolute path to data.json
    let mut data_path_buf = PathBuf::from(base_dir);
    data_path_buf.push("data.json");

    // 2. Resolve absolute path to settings.json 
    let mut settings_path_buf = PathBuf::from(base_dir);
    settings_path_buf.push("settings.json");

    // Initialize raw managers using the references
    let data_manager_raw = DataManager::new(&data_path_buf)
        .expect("Failed to initialize DataManager context");
        
    let settings_manager_raw = SettingsManager::new(&settings_path_buf)
        .expect("Failed to initialize SettingsManager context");

    // Wrap into final UI usable reference counting cells
    let data_manager = Rc::new(RefCell::new(data_manager_raw));
    let settings_manager = Rc::new(RefCell::new(settings_manager_raw));

    (data_manager, settings_manager)
}



*////



// ── Data layer (Baked Fallbacks & Auto-Creation) ─────────────────────────


pub fn initialize_managers() -> (Rc<RefCell<DataManager>>, Rc<RefCell<SettingsManager>>) {

 
    // 1. Bake the raw JSON template contents directly into the executable binary
    let baked_data_json = include_str!("../data.json");
    let baked_settings_json = include_str!("../settings.json");

    // 2. Locate the system config directory (~/.config on Linux/Mac, AppData\Roaming on Windows)
    let mut config_dir = dirs::config_dir()
        .expect("Could not find the system config directory");
    
    // Append your specific app folder path: .../film/
    config_dir.push("Film");

    // 3. Create the directory tree if it doesn't exist yet
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .expect("Failed to create .../film/ directory");
    }

    // 4. Resolve the absolute paths for both files
    let json_path = config_dir.join("data.json");
    let settings_path = config_dir.join("settings.json");

    // 5. If data.json doesn't exist, create it and fill it with the baked data
    if !json_path.exists() {
        std::fs::write(&json_path, baked_data_json)
            .expect("Failed to write default data.json to config folder");
    }

    // 6. If settings.json doesn't exist, create it and fill it with the baked data
    if !settings_path.exists() {
        std::fs::write(&settings_path, baked_settings_json)
            .expect("Failed to write default settings.json to config folder");
    }



    // Initialize raw managers using the references
    let data_manager_raw = DataManager::new(&json_path)
        .expect("Failed to initialize DataManager context");
        
    let settings_manager_raw = SettingsManager::new(&settings_path)
        .expect("Failed to initialize SettingsManager context");

    // Wrap into final UI usable reference counting cells
    let data_manager = Rc::new(RefCell::new(data_manager_raw));
    let settings_manager = Rc::new(RefCell::new(settings_manager_raw));

    (data_manager, settings_manager)




    // 7. Initialize your managers using the newly created/existing files on disk
    //let json_path_str = json_path.to_string_lossy();
    //let manager = data_manager::DataManager::new(&json_path_str)
    //    .expect("Failed to initialize JSON database");
    //let shared_manager = Rc::new(RefCell::new(manager));

    //let settings_path_str = settings_path.to_string_lossy();
    //let settings_mgr = data_manager::SettingsManager::new(&settings_path_str)
    //    .expect("Failed to initialize settings");
    //let shared_settings = Rc::new(RefCell::new(settings_mgr));
    

}







