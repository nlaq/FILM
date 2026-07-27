use slint::ComponentHandle;
use slint::Model;
use crate::AppWindow;
use futures::stream::{StreamExt, FuturesUnordered};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use log::{error, info, warn};
use crate::Thumbnail;


/// Thread-safe payload to communicate finished processing data points
#[derive(Debug)]
pub struct ProcessedItemMsg {
    raw_path: PathBuf,
    output_path: PathBuf,
    is_portrait: bool,
}

/// Type alias for scannability
type SharedPath = Arc<PathBuf>;

/// Strongly typed orientation variants to prevent string-matching bugs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Normal,    // Código EXIF 1
    Rotate90,  // Código EXIF 6
    Rotate180, // Código EXIF 3
    Rotate270, // Código EXIF 8
}

impl Orientation {
    /// Parse raw numeric orientation strings safely into a strongly typed variant
    pub fn from_str(s: &str) -> Self {
        // 🚀 CORRECCIÓN: Mapeo numérico estricto 1-a-1 con la salida de ExifTool -n
        match s.trim() {
            "6" => Orientation::Rotate90,   // Vertical (Rotar 90 CW)
            "3" => Orientation::Rotate180,  // Invertido (Rotar 180)
            "8" => Orientation::Rotate270,  // Vertical Inverso (Rotar 270 CW)
            _   => Orientation::Normal,     // "1" o cualquier valor por defecto (Horizontal)
        }
    }

    pub fn is_portrait(&self) -> bool {
        matches!(self, Orientation::Rotate90 | Orientation::Rotate270)
    }

    /// Convert variant back into format expected by your image_processing library
    pub fn to_str_legacy(&self) -> &'static str {
        match self {
            Orientation::Rotate90 => "Rotate 90 CW",
            Orientation::Rotate180 => "Rotate 180",
            Orientation::Rotate270 => "Rotate 270 CW",
            Orientation::Normal => "Horizontal (normal)",
        }
    }
}

//////////////////////////////////////////////////
////// Process for each RAW 
//////////////////////////////////////////////////
fn process_single_raw(
    exiftool_bin: &Path,
    raw_path: PathBuf,
    cache_dir: &Path,
    appending_name: &str, 
    extension: &str,      
    portrait_target_width: u32,
    landscape_target_width: u32,
    token: &CancellationToken,
) -> Option<ProcessedItemMsg> {

    if token.is_cancelled() { return None; }
    let file_stem = raw_path.file_stem()?.to_str()?.to_string();
    let output_path = cache_dir.join(format!("{}{}.{}", file_stem, appending_name, extension));

    // 1. Extract embedded preview bytes
    let raw_bytes = crate::exif_data::extract_preview_image(exiftool_bin, &raw_path)
        .map_err(|e| error!("Extraction failed for '{}': {}", file_stem, e)).ok()?;
    
    if token.is_cancelled() { return None; }

    // 2. Identify orientation profiles using type safety
    let orientation_str = crate::exif_data::determine_orientation(exiftool_bin, &raw_path)
        .unwrap_or_else(|_| "1".to_string());
    let orientation = Orientation::from_str(&orientation_str);

    let is_portrait = orientation.is_portrait();
    let target_width = if is_portrait { portrait_target_width } else { landscape_target_width };

    if token.is_cancelled() { return None; }
    
    // 3. Perform standard scaling operations
    let mut processed_bytes = crate::image_processing::resize_jpeg_by_width(&raw_bytes, target_width)
        .map_err(|e| error!("Scaling failed for '{}' with suffix '{}': {}", file_stem, appending_name, e)).ok()?;

    if token.is_cancelled() { return None; }

    // 4. Handle rotational corrections cleanly via match states
    if orientation != Orientation::Normal {
        processed_bytes = crate::image_processing::rotate_jpeg_bytes(&processed_bytes, orientation.to_str_legacy())
            .map_err(|e| error!("Rotation failure for '{}' with suffix '{}': {}", file_stem, appending_name, e)).ok()?;
    }

    if token.is_cancelled() { return None; }

    // 5. Commit mutations to disk cache
    std::fs::write(&output_path, processed_bytes)
        .map_err(|e| error!("IO Error saving file for '{}' with suffix '{}': {}", file_stem, appending_name, e)).ok()?;

    Some(ProcessedItemMsg {
        raw_path,
        output_path,
        is_portrait,
    })
}

/////////////////////////////////////////////////
//// PROCESS DIRECTORY PIPELINE
/////////////////////////////////////////////////

pub async fn process_directory_pipeline(
    ui_handle: slint::Weak<AppWindow>,
    selected_path: PathBuf,
    exiftool_bin: SharedPath,
    cache_dir: SharedPath,
    token: CancellationToken,
) {
    let suffix = "_thumbnail";
    let ext = "jpg";
    let portrait_w = 390;
    let landscape_w = 260;

    info!("[Pipeline] Resolving filesystem file targets asynchronously...");

    let mut raw_files = match tokio::task::spawn_blocking({
        let path = selected_path.clone();
        move || crate::image_processing::discover_raw_images(&path)
    }).await {
        Ok(Ok(files)) => files,
        Ok(Err(e)) => { error!("[Pipeline] Target discovery failed: {}", e); return; },
        Err(e) => { error!("[Pipeline] Operational panic during lookup: {}", e); return; }
    };
    
    if raw_files.is_empty() {
        warn!("[Pipeline] Zero RAW targets discovered.");
        return;
    }

    raw_files.sort_by(|a, b| a.cmp(b));
    if token.is_cancelled() { return; }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ProcessedItemMsg>(32);

    let ui_handle_clone = ui_handle.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_handle_clone.upgrade() {
            let ui_model = std::rc::Rc::new(slint::VecModel::<Thumbnail>::default());
            ui.set_file_list(slint::ModelRc::new(ui_model));
        }
    });

    let ui_handle_monitor = ui_handle.clone();
    let receiver_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let ui_weak = ui_handle_monitor.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    if let Ok(img) = slint::Image::load_from_path(&msg.output_path) {
                        let ui_thumb = Thumbnail {
                            path_thumbnail: slint::SharedString::from(msg.output_path.to_string_lossy().to_string()),
                            path_raw: slint::SharedString::from(msg.raw_path.to_string_lossy().to_string()), 
                            preview: img,
                            is_portrait: msg.is_portrait,
                            is_selected: false,
                        };

                        let model_rc = ui.get_file_list();
                        if let Some(vec_model) = model_rc.as_any().downcast_ref::<slint::VecModel<Thumbnail>>() {
                            vec_model.push(ui_thumb);
                        }
                    }
                }
            });
        }
    });

    let max_concurrent_tasks = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .min(4);
    
    let mut workers = FuturesUnordered::new();

    for raw_path in raw_files {
        if token.is_cancelled() { break; }

        if workers.len() >= max_concurrent_tasks {
            if let Some(Ok(Some(msg))) = workers.next().await {
                if tx.send(msg).await.is_err() { break; }
            }
        }

        let bin_ref = exiftool_bin.clone();
        let cache_ref = cache_dir.clone();
        let token_ref = token.clone();

        workers.push(tokio::task::spawn_blocking(move || {
            process_single_raw(
                &bin_ref, 
                raw_path, 
                &cache_ref,
                suffix, ext, portrait_w, landscape_w,
                &token_ref
            )
        }));
    }

    while let Some(result) = workers.next().await {
        if token.is_cancelled() { break; }
        if let Ok(Some(msg)) = result {
            if tx.send(msg).await.is_err() { break; }
        }
    }
    
    drop(tx);
    let _ = receiver_handle.await;
    info!("[Pipeline Complete] Processing loops terminated cleanly.");
}


////////////////////
///////STRUCTURED EXIF METADATA FOR UI
////////////////////


pub const KNOWN_CAMERA_BRANDS: &[&str] = &[
    "Canon",
    "Sony",
    "Nikon",
    "Fujifilm",
    "Panasonic",
    "Leica",
    "OM System",
    "Pentax",
    "Hasselblad",
    "Sigma",
    "Blackmagic Design",
    "GoPro",
    "Kodak",
    "Polaroid",
    "Ricoh",
    "Phase One",
    "RED",
    "DJI",
    "Insta360",
    "Kenko",
    "AgfaPhoto",
];


pub fn build_camera_display(
    make: Option<String>,
    model: Option<String>,
) -> String {

    let make = make
        .unwrap_or_else(|| "Unknown".to_string())
        .trim()
        .to_string();

    let model = model
        .unwrap_or_else(|| "Unknown".to_string())
        .trim()
        .to_string();


    if model == "Unknown" && make != "Unknown" {
        return make;
    }


    if make == "Unknown" && model != "Unknown" {
        return model;
    }


    let model_lower = model.to_lowercase();


    // Case 1:
    // Model already starts with Make
    //
    // Example:
    // Make: Canon
    // Model: Canon EOS R5
    //
    // Result:
    // Canon EOS R5
    //
    if model_lower.starts_with(
        &make.to_lowercase()
    ) {
        return model;
    }


    // Case 2:
    // Model starts with a known camera brand
    //
    // Example:
    // Make: SONY
    // Model: Sony ILCE-7M4
    //
    // Result:
    // Sony ILCE-7M4
    //
    for brand in KNOWN_CAMERA_BRANDS {

        if model_lower.starts_with(
            &brand.to_lowercase()
        ) {
            return model;
        }
    }


    // Default:
    // Make + Model
    //
    // Example:
    // Make: Nikon
    // Model: Z8
    //
    // Result:
    // Nikon Z8
    //
    format!(
        "{} {}",
        make,
        model
    )
}





pub fn exif_metadata_ui(bin_path: &std::path::Path, raw_path: &std::path::Path) -> (String, String, String, String, String) {
    let tags = &["-Make", "-Model", "-LensID", "-LensMake", "-LensModel", "-Aperture", "-ISO", "-ImageSize"];
    
    if let Ok(metadata) = crate::exif_data::extract_metadata(bin_path, raw_path, tags) {
        //let camera_make = metadata.camera_make.clone().unwrap_or_else(|| "Unknown".to_string());
        //let camera_model = metadata.camera_model.clone().unwrap_or_else(|| "Unknown".to_string());

        let camera_display = build_camera_display(
            metadata.camera_make.clone(),
            metadata.camera_model.clone(),
        );
        let lens_id = metadata.lens_id.clone().unwrap_or_else(|| "Unknown".to_string());
        let lens_make = metadata.lens_make.clone().unwrap_or_else(|| "Unknown".to_string());
        let lens_model_raw = metadata.lens_model.clone().unwrap_or_else(|| "Unknown".to_string());
        
// Helper check or inline array for placeholder values
        let generic_placeholders = [
            "Unknown", 
            "Other Lens or no lens", 
            "A-mount"
        ];
        
        let is_valid = |val: &str| -> bool {
            let trimmed = val.trim();
            if trimmed.is_empty() {
                return false;
            }
        
            // Convert the input string to lowercase once
            let lower_val = trimmed.to_lowercase();
        
            // Check if lower_val contains the lowercase version of any placeholder
            !generic_placeholders.iter().any(|ph| {
                let lower_ph = ph.to_lowercase();
                lower_val.contains(&lower_ph) // Requires the '&' reference
            })
        };
        
        let consolidated_lens = if is_valid(&lens_make) && is_valid(&lens_model_raw) {
            // 1. Prioritize LensMake and LensModel
            if crate::exif_data::lens_id_contains_known_brand(&lens_model_raw) {
                lens_model_raw.trim().to_string()
            } else {
                format!("{} {}", lens_make.trim(), lens_model_raw.trim())
            }
        } else if is_valid(&lens_id) {
            // 2. Fall back to LensID if it's NOT a generic string
            lens_id.trim().to_string()
        } else if is_valid(&lens_model_raw) {
            // 3. Fall back to raw model if present
            lens_model_raw.trim().to_string()
        } else {
            // 4. Ultimate fallback if no meaningful lens info exists
            "--".to_string()
        };

        let aperture_val = match metadata.aperture {
            Some(ap) => format!("f/{:.1}", ap),
            None => "--".to_string(),
        };

        let iso_val = metadata.iso.map(|i| i.to_string()).unwrap_or_else(|| "Unknown".to_string());
        let size_val = metadata.image_size.clone().unwrap_or_else(|| "Unknown".to_string());

        (camera_display, consolidated_lens, aperture_val, iso_val, size_val)
    } else {
        ("Unknown".into(), "Unknown".into(), "Unknown".into(), "Unknown".into(), "Unknown".into())
    }
}



/////////////////////////
///// PREVIEW UPDATE
/////////////////////////

pub fn preview_update(
    ui: &crate::AppWindow,
    idx: usize,
    exiftool_bin: &Path,
    cache_dir: &Path,
    rt_handle: &tokio::runtime::Handle, 
) {
    let file_list = ui.get_file_list();
    let clicked_raw_path = file_list.row_data(idx).map(|item| PathBuf::from(item.path_raw.as_str()));

    if let Some(raw_path) = clicked_raw_path {
        let ui_weak = ui.as_weak();
        let bin_ref = PathBuf::from(exiftool_bin);
        let cache_ref = PathBuf::from(cache_dir);
    
        rt_handle.spawn(async move {
            let suffix = "_preview";
            let ext = "jpg";
            let portrait_w = 1000;
            let landscape_w = 2000;
            
            let bin_clone = bin_ref.clone();
            let path_clone = raw_path.clone();
            
            // =========================================================
            // 1. EXTRACCIÓN UNIFICADA Y LIMPIA DE METADATOS (JSON)
            // =========================================================
            let metadata_payload = tokio::task::spawn_blocking(move || {
                // Llamamos a la calculadora común compartida con main.rs
                exif_metadata_ui(&bin_clone, &path_clone)
            }).await.unwrap_or_else(|_| (
                "Unknown".into(), "Unknown".into(), "Unknown".into(), "Unknown".into(), "Unknown".into()
            ));

            // =========================================================
            // 2. PROCESAMIENTO MÁXIMO DE IMAGEN / CACHÉ ORIGINAL
            // =========================================================
            let file_name = raw_path.file_stem().unwrap_or_default().to_string_lossy();
            let expected_path = cache_ref.join(format!("{}{}.{}", file_name, suffix, ext));
            let mut final_output_path: Option<PathBuf> = None;

            if expected_path.exists() {
                final_output_path = Some(expected_path);
            } else {
                let bin_fallback = bin_ref.clone();
                let cache_fallback = cache_ref.clone();
                let path_fallback = raw_path.clone();
                let dummy_token = tokio_util::sync::CancellationToken::new(); // o la ruta de tu struct CancellationToken
                
                if let Some(processed) = tokio::task::spawn_blocking(move || {
                    process_single_raw(&bin_fallback, path_fallback, &cache_fallback, suffix, ext, portrait_w, landscape_w, &dummy_token)
                }).await.unwrap_or(None) {
                    final_output_path = Some(processed.output_path);
                }
            }

            // =========================================================
            // 3. INYECCIÓN TOTAL Y ATÓMICA EN EL EVENT LOOP DE SLINT
            // =========================================================
            if let Some(output_path) = final_output_path {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        if let Ok(loaded_img) = slint::Image::load_from_path(&output_path) {
                            let app_data = ui.global::<crate::AppData>();
                            
                            // Inyectamos la imagen y limpiamos el texto
                            app_data.set_preview_image(loaded_img);
                            app_data.set_image_info_text("".into());
                            
                            // Inyectamos los 5 metadatos formateados del JSON
                            app_data.set_meta_camera(metadata_payload.0.into());
                            app_data.set_meta_lens(metadata_payload.1.into());
                            app_data.set_meta_aperture(metadata_payload.2.into());
                            app_data.set_meta_iso(metadata_payload.3.into());
                            app_data.set_meta_size(metadata_payload.4.into());
                        }    
                    }
                });
            }
        });
    }
}




///////////////////////////////////////////
///// CONTROLLER MANAGEMENT
//////////////////////////////////////////

#[derive(Default, Clone)]
pub struct PipelineController {
    active_task: Arc<Mutex<Option<(CancellationToken, JoinHandle<()>)>>>,
}

impl PipelineController {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn cancel_and_join(&self) {
        let maybe_task = self.active_task.lock().unwrap().take();
        if let Some((token, handle)) = maybe_task {
            info!("[PipelineController] Signaling active pipeline to abort...");
            token.cancel();
            let _ = handle.await;
            info!("[PipelineController] Old pipeline has completely shut down.");
        }
    }

pub async fn change_directory(
        &self,
        ui_handle: slint::Weak<AppWindow>,
        selected_path: PathBuf,
        exiftool_bin: Arc<PathBuf>,
        cache_dir: Arc<PathBuf>,
    ) {
        // 1. Cancel and join the previous processing task
        self.cancel_and_join().await;

        // 2. Clear the preview area immediately on the UI thread
        let ui_clear_handle = ui_handle.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_clear_handle.upgrade() {
                let app_data = ui.global::<crate::AppData>();
                
                // Reset the preview image to an empty/default image state
                app_data.set_preview_image(slint::Image::default());
                
                // Clear out text metadata values
                app_data.set_image_info_text("".into());
                app_data.set_meta_camera("".into());
                app_data.set_meta_lens("".into());
                app_data.set_meta_aperture("".into());
                app_data.set_meta_iso("".into());
                app_data.set_meta_size("".into());
            }
        });

        // 3. Run the non-critical cache cleanup
        if let Err(e) = crate::manage_dirs::clean_cache() {
            warn!("[Cache] Non-critical cache cleanup error: {}", e);
        }

        // 4. Spin up the new directory pipeline
        let new_token = CancellationToken::new();
        let token_clone = new_token.clone();

        let new_handle = tokio::spawn(async move {
            process_directory_pipeline(ui_handle, selected_path, exiftool_bin, cache_dir, token_clone).await;
        });

        *self.active_task.lock().unwrap() = Some((new_token, new_handle));
    }
}


//////////
///CTRL+CLICK HANDLER
///////////


pub fn handle_ctrl_click_preview(
    ui: &crate::AppWindow,
    target_idx: usize,
    exiftool_bin: &std::path::Path,
    cache_dir: &std::path::Path,
    rt_handle: &tokio::runtime::Handle,
) {
    let file_list = ui.get_file_list();
    let mut is_now_selected = false;

    // 1. Flip selection state on the clicked index
    if let Some(mut item) = file_list.row_data(target_idx) {
        item.is_selected = !item.is_selected;
        is_now_selected = item.is_selected;
        file_list.set_row_data(target_idx, item); 
    }

    // 2. Decide what preview to display
    if is_now_selected {
        // If selected, preview it directly
        preview_update(ui, target_idx, exiftool_bin, cache_dir, rt_handle);
    } else {
        // If deselected, scan BACKWARDS to find the last selected thumbnail fallback target
        let mut fallback_idx = None;
        
        // .rev() turns (0..count) into a backwards countdown (count-1 down to 0)
        for i in (0..file_list.row_count()).rev() {
            if let Some(item) = file_list.row_data(i) {
                if item.is_selected {
                    fallback_idx = Some(i);
                    break; // Found the last selected item in the list, stop scanning
                }
            }
        }

        if let Some(idx) = fallback_idx {
            preview_update(ui, idx, exiftool_bin, cache_dir, rt_handle);
        } else {
            // Nothing left selected: Clear out the metadata profile and image view ports
            let app_data = ui.global::<crate::AppData>();
            app_data.set_preview_image(slint::Image::default());
            app_data.set_image_info_text("".into());
            app_data.set_meta_camera("".into());
            app_data.set_meta_lens("".into());
            app_data.set_meta_aperture("".into());
            app_data.set_meta_iso("".into());
            app_data.set_meta_size("".into());
        }
    }
}


