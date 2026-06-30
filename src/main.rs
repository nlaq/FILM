use slint::{Model, SharedString};
mod img_processing;
mod show_preview;
mod data_manager;
mod new_lens;
mod remove_lens;
mod settings;
mod convert;
mod exif_process;
mod manage_cache;
use std::rc::Rc;
use std::cell::RefCell;

slint::include_modules!();

#[tokio::main] 
async fn main() -> Result<(), slint::PlatformError> {
    std::env::set_var("SLINT_COLOR_SCHEME", "light");

    let ui = AppWindow::new()?;
    let ui_handle = ui.as_weak();

    // Tracking vector to store structural memory IDs matching the UI index 1:1
    let tracking_ids = Rc::new(RefCell::new(Vec::<String>::new()));



// ── Data layer (Baked Fallbacks & Auto-Creation) ─────────────────────────
    
    // 1. Bake the raw JSON template contents directly into the executable binary
    let baked_data_json = include_str!("../data.json");
    let baked_settings_json = include_str!("../settings.json");

    // 2. Locate the system config directory (~/.config on Linux/Mac, AppData\Roaming on Windows)
    let mut config_dir = dirs::config_dir()
        .expect("Could not find the system config directory");
    
    // Append your specific app folder path: ~/.config/film/
    config_dir.push("film");

    // 3. Create the directory tree if it doesn't exist yet
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .expect("Failed to create ~/.config/film/ directory");
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

    // 7. Initialize your managers using the newly created/existing files on disk
    let json_path_str = json_path.to_string_lossy();
    let manager = data_manager::DataManager::new(&json_path_str)
        .expect("Failed to initialize JSON database");
    let shared_manager = Rc::new(RefCell::new(manager));

    let settings_path_str = settings_path.to_string_lossy();
    let settings_mgr = data_manager::SettingsManager::new(&settings_path_str)
        .expect("Failed to initialize settings");
    let shared_settings = Rc::new(RefCell::new(settings_mgr));
    
    
    
    // ── Populate UI lists ───────────────────────────────────────────────────
    {
        let manager_ref = shared_manager.borrow();
        let app_data = ui.global::<AppData>(); 

        let mut lenses: Vec<SharedString> = Vec::new();
        let mut ids = tracking_ids.borrow_mut();

        for l in &manager_ref.data.lenses {
            // Ensure the focal length always carries its unit suffix into the UI dropdown list
            let clean_focal = if l.focal.ends_with("mm") {
                l.focal.clone()
            } else {
                format!("{}mm", l.focal)
            };

            // FIXED: Initial combobox row layout displays ONLY max aperture
            lenses.push(SharedString::from(format!("{} {} {} f/{}", l.brand, l.model, clean_focal, l.max_aperture)));
            ids.push(l.id.clone()); 
        }
        app_data.set_lens_list(Rc::new(slint::VecModel::from(lenses)).into());

        let apertures: Vec<SharedString> = manager_ref.data.apertures
            .iter().map(|a| SharedString::from(format!("f/{}", a))).collect();
        app_data.set_aperture_list(Rc::new(slint::VecModel::from(apertures)).into());

        let brands: Vec<SharedString> = manager_ref.data.brands
            .iter().map(|b| SharedString::from(&b.brand_name)).collect();
        app_data.set_brand_list(Rc::new(slint::VecModel::from(brands)).into());

        let focals: Vec<SharedString> = manager_ref.data.focal_lengths
            .iter().map(|f| SharedString::from(f)).collect();
        app_data.set_focal_list(Rc::new(slint::VecModel::from(focals)).into());
    }

    // ── Simple menu callbacks ────────────────────────────────────────────────
    ui.on_settings_clicked({
        let shared_settings = Rc::clone(&shared_settings);
        move || {
            settings::show_settings_window(Rc::clone(&shared_settings));
        }
    });

    // ── On Convert Clicked - DNG Pipeline ────────────────────────────────────
    let ui_handle_convert = ui_handle.clone();
    let shared_settings_convert = Rc::clone(&shared_settings);

    ui.on_convert_clicked(move || {
        let Some(ui) = ui_handle_convert.upgrade() else { return };
        
        let Some((paths, folder, params, override_files)) = 
            convert::conversion_setup(&ui, &shared_settings_convert) else { return };

        ui.set_is_converting(true);
        
        let thread_ui_handle = ui_handle_convert.clone();

        tokio::spawn(async move {
            use dnglab::jobs::Job;

            for file_for_conv in paths {
                let mut output_path = folder.clone();
                if let Some(stem) = file_for_conv.file_stem() {
                    output_path.push(format!("{}.dng", stem.to_string_lossy()));
                } else {
                    continue;
                }

                let job = dnglab::jobs::raw2dng::Raw2DngJob {
                    input: file_for_conv.clone(),
                    output: output_path.clone(),
                    replace: override_files,
                    params: params.clone(),
                };

                let result = job.execute().await;
                if let Some(err) = result.error {
                    eprintln!("[ERROR] Raw2Dng engine crashed on file {:?}: {}", file_for_conv, err);
                    continue;
                }

                match exif_process::exiftool_repair(&file_for_conv, &output_path) {
                    Ok(msg) => println!("{}", msg),
                    Err(e) => eprintln!("Conversion hit 100% but metadata patch failed: {}", e),
                }
            }

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui_window) = thread_ui_handle.upgrade() {
                    ui_window.set_is_converting(false);
                }
            });
            println!("[INFO] Entire pipeline execution terminated.");
        });
    });

    // ── Open New Lens Window (Edit menu) ─────────────────────────────────────
    ui.on_open_new_lens_clicked({
        let shared_manager = Rc::clone(&shared_manager);
        let ui_handle      = ui_handle.clone();
        move || {
            new_lens::show_new_lens_window(
                Rc::clone(&shared_manager),
                ui_handle.clone(),
            );
        }
    });

    // ── Open Remove Window (Edit menu) ───────────────────────────────────────
    ui.on_open_remove_window_clicked({
        let shared_manager = Rc::clone(&shared_manager);
        let ui_handle      = ui_handle.clone();
        move || {
            remove_lens::show_remove_window(
                Rc::clone(&shared_manager),
                ui_handle.clone(),
            );
        }
    });
    
    // ── on_select_directory_clicked ──────────────────────────────────────────
    ui.on_select_directory_clicked({
        let ui_handle = ui_handle.clone();
        move || {
            let Some(ui_window) = ui_handle.upgrade() else { return };
            let Some(folder) = rfd::FileDialog::new().pick_folder() else { return };
            
            let app_data = ui_window.global::<AppData>();
            app_data.set_meta_camera("".into());
            app_data.set_meta_lens("".into());
            app_data.set_meta_aperture("".into());
            app_data.set_meta_iso("".into());
            app_data.set_meta_size("".into());

            app_data.set_image_info_text("Select an image".into());
            app_data.set_preview_image(slint::Image::default());
            app_data.set_file_selected_write_button_enabled(false);
            
            let _ = manage_cache::clean_cache();

            let initial_vec = Rc::new(slint::VecModel::<Thumbnail>::default());
            ui_window.set_file_list(slint::ModelRc::from(initial_vec));

            let thread_ui_handle = ui_handle.clone();
            let jpeg_width: u32 = 200;

            tokio::task::spawn_blocking(move || {
                let Ok(raw_files) = img_processing::find_raw_files(&folder) else { return };

                for path_raw in raw_files {
                    if let Some(preview) = img_processing::parse_raw_preview(&path_raw) {
                        if let Ok((path_jpg_cache, orientation)) =
                            img_processing::save_jpeg_with_exif(&path_raw, preview, jpeg_width)
                        {
                            let is_portrait   = matches!(orientation, 5 | 6 | 7 | 8);
                            let path_raw_str  = path_raw.to_string_lossy().into_owned();
                            let path_jpg_str  = path_jpg_cache.to_string_lossy().into_owned();
                            let loop_ui_handle = thread_ui_handle.clone();

                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui_window) = loop_ui_handle.upgrade() {
                                    if let Ok(slint_image) = slint::Image::load_from_path(
                                        std::path::Path::new(&path_jpg_str)
                                    ) {
                                        let new_thumbnail = Thumbnail {
                                            path_thumbnail: path_jpg_str.into(),
                                            path_raw: path_raw_str.into(),
                                            preview: slint_image,
                                            is_portrait,
                                            is_selected: false,
                                        };

                                        let current_model = ui_window.get_file_list();
                                        if let Some(vec_model) = current_model
                                            .as_any()
                                            .downcast_ref::<slint::VecModel<Thumbnail>>()
                                        {
                                            vec_model.push(new_thumbnail);
                                        }
                                    }
                                }
                            });
                        }
                    }
                }

                let final_ui_handle = thread_ui_handle.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui_window) = final_ui_handle.upgrade() {
                        ui_window.global::<AppData>()
                            .set_image_info_text("Select an image".into());
                    }
                });
            });
        }
    });

    // ── ON UI.FILE_SELECTED::::File selection helpers ────────────────────────
    fn sync_selection(model: &slint::ModelRc<Thumbnail>, selected: &[i32]) {
        for i in 0..model.row_count() {
            let mut thumb    = model.row_data(i).unwrap();
            let was_selected = thumb.is_selected;
            thumb.is_selected = selected.contains(&(i as i32));
            if thumb.is_selected != was_selected {
                model.set_row_data(i, thumb);
            }
        }
    }

    let selected_indices: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
    let current_lens:     Rc<RefCell<String>>   = Rc::new(RefCell::new(String::new()));
    let current_aperture: Rc<RefCell<String>>   = Rc::new(RefCell::new(String::new()));

    ui.on_file_selected({
        let selected_indices = selected_indices.clone();
        let ui_handle        = ui_handle.clone();
        move |index| {
            let Some(ui) = ui_handle.upgrade() else { return };
            
            let file_list = ui.get_file_list();
            let mut path_raw_str = String::new();
            if let Some(selected_item) = file_list.row_data(index as usize) {
                path_raw_str = selected_item.path_raw.to_string();
            }
            
            {
                let mut sel = selected_indices.borrow_mut();
                sel.clear();
                sel.push(index);
                sync_selection(&ui.get_file_list(), &sel);
            }
            show_preview::show(ui_handle.clone(), index);
            
            if !path_raw_str.is_empty() {
                println!("Reading EXIF for raw file path: {}", path_raw_str);
                let loop_ui_handle = ui_handle.clone();
                
                let _ = tokio::task::spawn_blocking(move || {
                    let path = std::path::PathBuf::from(path_raw_str);
                    let exif_data = exif_process::extract_important_exif(&path);
                    
                    println!("Extracted EXIF Data: {:?}", exif_data);

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui_window) = loop_ui_handle.upgrade() {
                            let app_data = ui_window.global::<AppData>();
                            app_data.set_meta_camera(exif_data.camera.into());
                            app_data.set_meta_lens(exif_data.lens.into());
                            app_data.set_meta_aperture(exif_data.aperture.into());
                            app_data.set_meta_iso(exif_data.iso.into());
                            app_data.set_meta_size(exif_data.size.into());
                        }
                    });
                });
            } else {
                println!("Warning: path_raw_str was resolved empty for index {}", index);
            }
        }
    });

    ui.on_thumbnail_ctrl_clicked({
        let selected_indices = selected_indices.clone();
        let ui_handle        = ui_handle.clone();
        move |index| {
            let Some(ui) = ui_handle.upgrade() else { return };
            let is_empty;
            {
                let mut sel = selected_indices.borrow_mut();
                if let Some(pos) = sel.iter().position(|&i| i == index) {
                    sel.remove(pos);
                } else {
                    sel.push(index);
                }
                sync_selection(&ui.get_file_list(), &sel);
                is_empty = sel.is_empty();
            }
            if is_empty {
                ui.global::<AppData>().set_preview_image(Default::default());
                ui.global::<AppData>().set_image_info_text("Seleccione una imagen".into());
                ui.global::<AppData>().set_file_selected_write_button_enabled(false);
            } else {
                show_preview::show(ui_handle.clone(), index);
            }
        }
    });

    // ── Lens combobox ────────────────────────────────────────────────────────
    ui.on_lens_changed({
        let ui_handle        = ui_handle.clone();
        let current_lens     = current_lens.clone();
        let current_aperture = current_aperture.clone();
        let shared_manager   = Rc::clone(&shared_manager);
        move |lens_name| {
            let Some(ui) = ui_handle.upgrade() else { return };
            println!("Selected Lens Mask string: {}", lens_name);

            let manager = shared_manager.borrow();
            let app_data = ui.global::<AppData>();

            let matched_lens = manager.data.lenses.iter().find(|l| {
                let clean_focal = if l.focal.ends_with("mm") { l.focal.clone() } else { format!("{}mm", l.focal) };
                // FIXED: Match string format template must strictly use max aperture only
                let current_formatted = format!("{} {} {} f/{}", l.brand, l.model, clean_focal, l.max_aperture);
                current_formatted == lens_name.as_str()
            });

            if let Some(matched_lens) = matched_lens {
                let all_apertures = &manager.data.apertures;

                let min_idx = all_apertures.iter().position(|a| a == &matched_lens.max_aperture);
                let max_idx = all_apertures.iter().position(|a| a == &matched_lens.min_aperture);

                if let (Some(start), Some(end)) = (min_idx, max_idx) {
                    let range_start = start.min(end);
                    let range_end   = start.max(end);

                    let filtered_apertures: Vec<SharedString> = all_apertures[range_start..=range_end]
                        .iter().map(|a| SharedString::from(format!("f/{}", a))).collect();

                    if let Some(first_ap) = filtered_apertures.first() {
                        *current_aperture.borrow_mut() = first_ap.to_string().trim_start_matches("f/").to_string();
                        app_data.set_aperture_selected_write_button_enabled(true);
                    }

                    app_data.set_aperture_list(
                        Rc::new(slint::VecModel::from(filtered_apertures)).into()
                    );
                }
            }

            *current_lens.borrow_mut() = lens_name.to_string();
            app_data.set_lens_selected_write_button_enabled(true);
        }
    });

    // ── Aperture combobox ────────────────────────────────────────────────────
    ui.on_aperture_changed({
        let current_aperture = current_aperture.clone();
        let ui_handle        = ui_handle.clone();
        move |aperture_val| {
            let Some(ui) = ui_handle.upgrade() else { return };
            let app_data = ui.global::<AppData>();
            if !aperture_val.is_empty() && aperture_val != "Select..." {
                let bare = aperture_val.trim_start_matches("f/").to_string();
                *current_aperture.borrow_mut() = bare;
                app_data.set_aperture_selected_write_button_enabled(true);
            } else {
                *current_aperture.borrow_mut() = String::new();
                app_data.set_aperture_selected_write_button_enabled(false);
            }
        }
    });

    // ── WRITE EXIF ──────────────────────────────────────────────────────────
    let ui_handle_exif = ui_handle.clone();
    let data_manager_exif = shared_manager.clone();
    let tracking_ids_exif = tracking_ids.clone();

    let current_lens_exif = current_lens.clone();
    let current_aperture_exif = current_aperture.clone();
    let selected_indices_exif = selected_indices.clone();

    ui.on_write_exif_clicked(move || {
        let Some(ui) = ui_handle_exif.upgrade() else { return };
        
        let lens_ui_string = current_lens_exif.borrow().clone();
        let aperture_snapshot = current_aperture_exif.borrow().clone();
        let indices_snapshot = selected_indices_exif.borrow().clone();
        
        let file_list = ui.get_file_list();
        let manager = data_manager_exif.borrow();
        let app_data = ui.global::<AppData>();

        // Find the index position where this exact layout string lives in the Slint Model view array
        let lens_list_model = app_data.get_lens_list();
        let mut target_index: Option<usize> = None;
        
        for i in 0..lens_list_model.row_count() {
            if let Some(val) = lens_list_model.row_data(i) {
                if val.to_string() == lens_ui_string {
                    target_index = Some(i);
                    break;
                }
            }
        }

        let Some(idx) = target_index else {
            eprintln!("[ERROR] Synchronization failure. Selected combo element outside array scope.");
            return;
        };

        // Extract the hidden unique memory ID using that same structural coordinate match
        let target_id = tracking_ids_exif.borrow()[idx].clone();

        // Query DataManager directly by ID string
        let Some(lens_data) = manager.get_lens_by_id(&target_id) else {
            eprintln!("[ERROR] Structural match failed for internal target ID: {}", target_id);
            return;
        };

        // Format focal length nicely for EXIF and UI (ensuring it contains "mm")
        let clean_focal = if lens_data.focal.ends_with("mm") {
            lens_data.focal.clone()
        } else {
            format!("{}mm", lens_data.focal)
        };

        // FIXED: UI sidebar layout display uses max aperture format explicitly
        let clean_ui_sidebar_string = format!(
            "{} {} {} f/{}", 
            lens_data.brand, 
            lens_data.model, 
            clean_focal, 
            lens_data.max_aperture
        );

        for &index in indices_snapshot.iter() {
            let idx_usize = index as usize;
            
            if let Some(file_list_item) = file_list.row_data(idx_usize) {
                let raw_path_str = file_list_item.path_raw.to_string();
                let path = std::path::Path::new(&raw_path_str);

                println!("[INFO] Injecting EXIF into: {:?}", path.file_name().unwrap_or_default());
                
                // Pass clean database strings directly down to the writing operation
                match exif_process::write_lens_and_aperture_metadata(
                    path, 
                    &lens_data.brand,
                    &lens_data.model,
                    &clean_focal,
                    &lens_data.max_aperture,
                    &aperture_snapshot
                ) {
                    Ok(_) => {
                        file_list.set_row_data(idx_usize, file_list_item);

                        // Update sidebar immediately if a single image is active
                        if indices_snapshot.len() == 1 {
                            app_data.set_meta_lens(clean_ui_sidebar_string.clone().into());
                            if !aperture_snapshot.is_empty() {
                                app_data.set_meta_aperture(format!("f/{}", aperture_snapshot).into());
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("[ERROR] Metadata write failure for file {:?}: {}", path, e);
                    }
                }
            }
        }
        println!("[INFO] Metadata write pipeline complete. UI updated successfully.");
    }); 
    
    // ── Intercept Window Close Event ─────────────────────────────────────────
    ui.window().on_close_requested(move || {
        println!("[INFO] Close requested. Running cache cleanup pipeline...");
        
        let _ = manage_cache::clean_cache();

        // Return CloseResponse::KeepWindowHidden to let the window shut down normally.
        // (You can also return CloseResponse::CancelClose if you wanted to stop it)
        slint::CloseRequestResponse::HideWindow
    });
    
    
       
    ui.run()
}