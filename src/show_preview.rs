use slint::ComponentHandle;
use slint::Model;
use crate::{AppWindow, AppData};

pub fn show(ui_handle: slint::Weak<AppWindow>, index: i32) {
    let Some(ui) = ui_handle.upgrade() else { return; };

    let image_list = ui.get_file_list();
    let Some(selected_item) = image_list.row_data(index as usize) else { return; };

    ui.global::<AppData>().set_image_info_text(selected_item.path_raw.clone());
    ui.global::<AppData>().set_file_selected_write_button_enabled(true);

    let path_raw = std::path::PathBuf::from(selected_item.path_raw.to_string());
    let path_thumbnail = std::path::PathBuf::from(selected_item.path_thumbnail.to_string());

    // Build the expected preview path the same way save_jpeg_with_exif does
    let mut preview_path = path_thumbnail.clone();
    if let Some(file_stem) = path_thumbnail.file_stem() {
        let mut new_name = file_stem.to_os_string();
        new_name.push("_preview.jpg");
        preview_path.set_file_name(new_name);
    }

    std::thread::spawn(move || {
        let final_path = if preview_path.exists() {
            println!("Cache hit: {:?}", preview_path);
            Some(preview_path)
        } else {
            println!("Cache miss: {:?}", path_raw);
            crate::img_processing::parse_raw_preview(&path_raw)
                .and_then(|preview| {
                    crate::img_processing::save_jpeg_with_exif(&path_raw, preview, 2000)
                        .ok()
                        .map(|(path, _)| path)
                })
        };

        if let Some(ready_path) = final_path {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui_window) = ui_handle.upgrade() {
                    if let Ok(slint_image) = slint::Image::load_from_path(&ready_path) {
                        ui_window.global::<AppData>().set_preview_image(slint_image);
                    }
                }
            });
        }
    });
}