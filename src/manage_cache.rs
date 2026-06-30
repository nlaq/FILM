use std::path::{PathBuf};

//////// RUTA PARA EL CACHE
pub fn obtener_ruta_cache() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut cache_path = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("No se pudo determinar la carpeta de usuario del entorno UNIX")?;
    cache_path.push(".cache");
    cache_path.push("film");
    Ok(cache_path)
}


pub fn clean_cache() -> Result<(), Box<dyn std::error::Error>> {
    let cache_path = obtener_ruta_cache()?;
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
