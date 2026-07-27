mod new_lens;
mod remove_lens;
mod convert;
mod data_manager;
mod exif_data;
mod image_processing;
mod models;
mod thumbnails_previews;
mod manage_dirs;
mod settings;
mod ui_update;
mod exif_inject;

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use log::{error, info, warn};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use data_manager::{DataManager, SettingsManager};
use settings::SettingsController;
use remove_lens::RemoveLensController;
use new_lens::NewLensController;


slint::include_modules!();

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("[Film] Initializing framework and backend controller contexts...");
    
    // Build the multi-threaded Tokio runtime context
    let tokio_runtime = Rc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to build multi-threaded Tokio runtime engine context")
    );
    let rt_handle = tokio_runtime.handle().clone();
      
    // Initialize Pipeline Controller & Exiftool
    let pipeline_controller = crate::thumbnails_previews::PipelineController::new();
    let exiftool_bin = match crate::exif_data::ensure_embedded_exiftool() {
        Ok(bin_path) => Arc::new(bin_path),
        Err(e) => panic!("Runtime extraction failure: {}", e),
    };
    
    // Manage directories & data
    let cache_dir = crate::manage_dirs::initialize_cache();
    let (data_manager, settings_manager) = crate::manage_dirs::initialize_managers();

    // Build main Window
    let ui = AppWindow::new().expect("Failed to initialize main AppWindow layout context");
    let ui_handle = ui.as_weak();
    
    // Setup state variables
    let selected_lens_index: Rc<RefCell<Option<usize>>> = Rc::new(RefCell::new(None));
    let selected_aperture: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    
    // Initial UI load
    crate::ui_update::refresh_lens_lists(&ui, &data_manager, &selected_lens_index);
        
    // Instantiate sub-controllers
    let remove_controller = RemoveLensController::new(ui.as_weak().upgrade().unwrap(), data_manager.clone());    
    let new_lens_controller = NewLensController::new(ui.as_weak().upgrade().unwrap(), data_manager.clone());
    let settings_controller = SettingsController::new(ui.as_weak().upgrade().unwrap(), settings_manager.clone());

    // =========================================================================
    // GUI Callback Handlers
    // =========================================================================

    // List Refresh
    ui.on_refresh_lists({
        let dm_for_refresh = data_manager.clone();
        let index_for_refresh = selected_lens_index.clone();
        let ui_handle_refresh = ui_handle.clone();
        move || {
            if let Some(ui) = ui_handle_refresh.upgrade() {
                crate::ui_update::refresh_lens_lists(&ui, &dm_for_refresh, &index_for_refresh);
            }
        }
    });

    // Directory Selection
    ui.on_select_directory_clicked({
        let ui_handle = ui_handle.clone();
        let exiftool_bin = exiftool_bin.clone();
        let cache_dir = cache_dir.clone();
        let runtime = tokio_runtime.clone(); 
        let controller = pipeline_controller.clone();
        
        move || {
            if let Some(selected_path) = rfd::FileDialog::new().pick_folder() {
                info!("[GUI] User selected directory: {}", selected_path.display());
                
                let ui_clone = ui_handle.clone();
                let exif_bin = exiftool_bin.clone();
                let cache = cache_dir.clone();
                let controller_clone = controller.clone();
    
                runtime.spawn(async move {
                    controller_clone.change_directory(ui_clone, selected_path, exif_bin, cache).await;
                });
            } else {
                warn!("[GUI] Directory picking canceled by user.");
            }
        }
    });  
    
    // Lens selection changed
    ui.on_lens_changed({
        let ui_handle = ui_handle.clone();
        let dm = data_manager.clone();
        let selected_lens_index = selected_lens_index.clone();
    
        move |lens_index| {
            if let Some(ui) = ui_handle.upgrade() {
                let index = lens_index as usize;
                info!("[GUI Callback] Lens index selected: {}", index);
                *selected_lens_index.borrow_mut() = Some(index);
    
                let apertures = {
                    let dm = dm.borrow();
                    let lenses = dm.get_sorted_lenses();
                    match lenses.get(index) {
                        Some(lens) => dm.get_apertures_for_lens(lens),
                        None => Vec::new(),
                    }
                };
    
                let model = ModelRc::new(VecModel::from(
                    apertures.into_iter().map(SharedString::from).collect::<Vec<_>>(),
                ));
    
                let app_data = ui.global::<AppData>();
                app_data.set_aperture_list(model);
                app_data.set_lens_selected_write_button_enabled(true);
            }
        }
    });

    // Aperture Selection Changed
    ui.on_aperture_changed({
        let ui_handle = ui_handle.clone();
        let selected_aperture = selected_aperture.clone();
    
        move |aperture| {
            if let Some(ui) = ui_handle.upgrade() {
                info!("[GUI Callback] Shooting aperture selected: f/{}", aperture.as_str());
                *selected_aperture.borrow_mut() = Some(aperture.to_string());
                ui.global::<AppData>().set_aperture_selected_write_button_enabled(true);
            }
        }
    });
    
    // Write Exif button
    ui.on_write_exif_clicked({
        let ui_handle = ui_handle.clone();
        let data_manager = data_manager.clone();
        let exiftool_bin = exiftool_bin.clone();
        let selected_lens_index = selected_lens_index.clone();
        let selected_aperture = selected_aperture.clone();
        let rt_handle = rt_handle.clone();
    
        move || {
            let ui = match ui_handle.upgrade() {
                Some(ui) => ui,
                None => return,
            };
    
            info!("[GUI Callback] Write Exif requested.");
    
            // Extraer y validar los parámetros (Todo movido a exif_inject)
            let params = match crate::exif_inject::extract_params_from_ui(
                &ui,
                &data_manager,
                &selected_lens_index,
                &selected_aperture,
            ) {
                Some(p) => p,
                None => return, // Si falló la validación, salimos silenciosamente (el log ya se lanzó dentro)
            };
    
            let ui_weak = ui_handle.clone();
            let bin_async = exiftool_bin.clone(); // Previene el error FnMut de Rust
    
            // Disparar proceso asíncrono
            rt_handle.spawn(async move {
                if let Some(meta) = crate::exif_inject::process_and_fetch_metadata(params, bin_async).await {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui_inc) = ui_weak.upgrade() {
                            let app_data = ui_inc.global::<crate::AppData>();
                            app_data.set_meta_camera(meta.0.into());
                            app_data.set_meta_lens(meta.1.into());
                            app_data.set_meta_aperture(meta.2.into());
                            app_data.set_meta_iso(meta.3.into());
                            app_data.set_meta_size(meta.4.into());
                        }
                    });
                }
            });
        }
    });

    // Convert to DNG button
    ui.on_convert_clicked({
        let ui_handle = ui_handle.clone();
        let settings_manager = settings_manager.clone();
        let runtime = tokio_runtime.clone();
        let conversion_controller = crate::convert::ConversionController::new();

        move || {
            let ui = match ui_handle.upgrade() {
                Some(ui) => ui,
                None => {
                    error!("[Convert] Failed to recover UI handle.");
                    return;
                }
            };

            info!("[GUI Callback] Convert to DNG requested.");

            let request = match crate::convert::conversion_setup(&ui, &settings_manager) {
                Some(request) => request,
                None => {
                    warn!("[Convert] Conversion request could not be created.");
                    return;
                }
            };

            ui.set_is_converting(true);
            let controller = conversion_controller.clone();
            let ui_weak = ui_handle.clone();

            runtime.spawn(async move {
                let failed = controller.convert(request).await;
            
                let _ = slint::invoke_from_event_loop(move || {
                    crate::ui_update::report_conversion_result(ui_weak, failed);
                });
            });
        }
    });



    // Thumbnail regular Left-Click    


// Creamos una celda para el ancla persistente del Shift+Click
// Modifica tu bloque on_file_selected para recibir el parámetro extra "action"
let anchor_index = Rc::new(Cell::new(0_usize));

ui.on_file_selected({
    let ui_handle = ui_handle.clone();
    let exiftool_bin = exiftool_bin.clone(); 
    let cache_dir = cache_dir.clone();
    let rt_handle = rt_handle.clone();
    let anchor = anchor_index.clone();

    move |index, action| {
        if let Some(ui) = ui_handle.upgrade() {
            let target_idx = index as usize;
            let file_list = ui.get_file_list();
            
            match action.as_str() {
                // --- REQUISITO: CTRL + CLICK ---
                "ctrl-click" => {
                    crate::thumbnails_previews::handle_ctrl_click_preview(
                        &ui, target_idx, &exiftool_bin, &cache_dir, &rt_handle
                    );

                    if let Some(mut item) = file_list.row_data(target_idx) {
                        item.is_selected = !item.is_selected;
                        file_list.set_row_data(target_idx, item);
                    }
                    anchor.set(target_idx);
                },

                // --- REQUISITO: SHIFT + CLICK o SHIFT + FLECHAS ---
                // Ambas acciones expanden el rango matemáticamente desde el ancla actual
                "shift-click" | "arrow-shift" => {
                    let start = anchor.get().min(target_idx);
                    let end = anchor.get().max(target_idx);

                    for i in 0..file_list.row_count() {
                        if let Some(mut item) = file_list.row_data(i) {
                            item.is_selected = i >= start && i <= end;
                            file_list.set_row_data(i, item);
                        }
                    }
                    
                    crate::thumbnails_previews::preview_update(
                        &ui, target_idx, &exiftool_bin, &cache_dir, &rt_handle
                    );
                },

                // --- REQUISITO: CTRL + FLECHAS ---
                // Como en el explorador de archivos nativo, Ctrl+Flechas solo desplaza el foco visual
                // (el recuadro gris), pero NO altera las selecciones existentes de la lista.
                "arrow-ctrl" => {
                    // Se deja vacío intencionalmente para preservar la selección previa
                },

                // --- ESCENARIO BASE: CLICK NORMAL o FLECHA LIMPIA ---
                _ => { // Absorbe "click" y "arrow" normal
                    for i in 0..file_list.row_count() {
                        if let Some(mut item) = file_list.row_data(i) {
                            item.is_selected = i == target_idx;
                            file_list.set_row_data(i, item);
                        }
                    }
                    anchor.set(target_idx);

                    crate::thumbnails_previews::preview_update(
                        &ui, target_idx, &exiftool_bin, &cache_dir, &rt_handle
                    );
                }
            }

            // Sincronización del estado del botón global
            let has_selection = (0..file_list.row_count()).any(|i| {
                file_list.row_data(i).map_or(false, |item| item.is_selected)
            });
            ui.global::<AppData>().set_file_selected_write_button_enabled(has_selection);
        }
    }
});



ui.on_thumbnail_ctrl_clicked({
        let ui_handle = ui_handle.clone();
        let exiftool_bin = exiftool_bin.clone();
        let cache_dir = cache_dir.clone();
        let rt_handle = rt_handle.clone();
    
        move |index| {
            if let Some(ui) = ui_handle.upgrade() {
                crate::thumbnails_previews::handle_ctrl_click_preview(
                    &ui, index as usize, &exiftool_bin, &cache_dir, &rt_handle
                );
        
                let file_list = ui.get_file_list();
                let mut has_selection = false;
        
                for i in 0..file_list.row_count() {
                    if let Some(item) = file_list.row_data(i) {
                        if item.is_selected {
                            has_selection = true;
                            break;
                        }
                    }
                }
                ui.global::<AppData>().set_file_selected_write_button_enabled(has_selection);
            }
        }
    });





    // Menus & Dialogue Openers
    ui.on_new_aperture_clicked(|| {
        info!("[GUI Callback] New aperture setup registration window trigger.");
    });

    let settings_controller_clone = settings_controller.clone();
    ui.on_settings_clicked(move || {
        info!("[GUI Callback] 'DNG Settings' requested.");
        settings_controller_clone.show_window();
    });    
    
    let new_lens_controller_clone = new_lens_controller.clone();
    ui.on_open_new_lens_clicked(move || {
        info!("[GUI] Open New Lens requested.");
        new_lens_controller_clone.show_window();    
    });    
    
    let remove_controller_clone = remove_controller.clone();
    ui.on_open_remove_window_clicked(move || {
        info!("[GUI] 'Remove Lens' window requested.");
        remove_controller_clone.show_window();
    });
    
    // Safe shutdown on close request (simply forward to standard exit path)
    ui.window().on_close_requested(move || {
        info!("[Film] Close request received. Initiating shutdown...");
        slint::CloseRequestResponse::HideWindow
    });

    // Run the UI main event loop (this blocks until closed)
    if let Err(e) = ui.run() {
        error!("Slint runtime execution failure encountered: {}", e);
    }

    // =========================================================================
    // CLEAN SHUTDOWN (Single point of execution after UI exits)
    // =========================================================================
    info!("[Film] UI closed. Stopping background workers before final cleanup...");
    tokio_runtime.block_on(async {
        pipeline_controller.cancel_and_join().await;
    });

    info!("[Film] Background workers joined. Performing shutdown cache cleanup...");
    if let Err(e) = crate::manage_dirs::clean_cache() {
        warn!("[Cache] Shutdown cache wipe bypassed: {}", e);
    } else {
        info!("[Cache] Shutdown cache wiped cleanly.");
    }
}