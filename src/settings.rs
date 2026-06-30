use slint::ComponentHandle;
use slint::Global;
use std::rc::Rc;
use std::cell::RefCell;
use crate::data_manager::SettingsManager;
use crate::SettingsWindow;
use crate::DngSettings;

/// Opens the Settings window. On Save, writes each field through SettingsManager
/// (which auto-saves to settings.json), mirroring how new_lens.rs uses DataManager.
pub fn show_settings_window(shared_settings: Rc<RefCell<SettingsManager>>) {
    let win = SettingsWindow::new().expect("Failed to create Settings window");
    let win_handle = win.as_weak();

    // Populate the Slint global from the currently loaded settings
    {
        let s = shared_settings.borrow();
        let g = DngSettings::get(&win);
        g.set_artist(s.data.artist.clone().into());
        g.set_compression(s.data.compression.clone().into());
        g.set_crop(s.data.crop.clone().into());
        g.set_dng_preview(s.data.dng_preview);
        g.set_dng_thumbnail(s.data.dng_thumbnail);
        g.set_embed_raw(s.data.embed_raw);
        g.set_override_files(s.data.override_files);
        g.set_image_index(s.data.image_index.clone().into());
        // ComboBox is 0-indexed; predictor values are 1–7
        g.set_ljpeg92_predictor(s.data.ljpeg92_predictor.saturating_sub(1) as i32);
    }

    // Save: call each setter on SettingsManager (mutates + auto-saves to disk)
    win.on_save_clicked({
        let win_handle      = win_handle.clone();
        let shared_settings = shared_settings.clone();
        move || {
            let Some(win) = win_handle.upgrade() else { return };
            let g = DngSettings::get(&win);

            let raw_index = g.get_image_index().to_string();
            let image_index = if raw_index.trim() == "all"
                || raw_index.trim().parse::<u32>().is_ok()
            {
                raw_index.trim().to_string()
            } else {
                "0".to_string()
            };

            // predictor: ComboBox 0-based index → 1-based value
            let predictor = (g.get_ljpeg92_predictor() + 1).clamp(1, 7) as u8;

            let mut s = shared_settings.borrow_mut();
            let _ = s.set_artist(g.get_artist().to_string());
            let _ = s.set_compression(g.get_compression().to_string());
            let _ = s.set_crop(g.get_crop().to_string());
            let _ = s.set_dng_preview(g.get_dng_preview());
            let _ = s.set_dng_thumbnail(g.get_dng_thumbnail());
            let _ = s.set_embed_raw(g.get_embed_raw());
            let _ = s.set_override_files(g.get_override_files());
            let _ = s.set_image_index(image_index);
            let _ = s.set_ljpeg92_predictor(predictor);

            if let Some(w) = win_handle.upgrade() { let _ = w.hide(); }
        }
    });

    win.on_cancel_clicked({
        let win_handle = win_handle.clone();
        move || {
            if let Some(w) = win_handle.upgrade() { let _ = w.hide(); }
        }
    });

    win.show().expect("Failed to show Settings window");
    std::mem::forget(win);
}
