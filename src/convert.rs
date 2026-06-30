use std::path::{PathBuf};
use std::rc::Rc;
use std::cell::RefCell;
use crate::data_manager::SettingsManager;
use slint::Model;
use rawler::dng::convert::ConvertParams;
use rawler::dng::{DngCompression, CropMode, DngPhotometricConversion};

// ── WORKER 1: Pure conversion parameter builder ─────────────────────────────
pub fn conversion_setup(
    ui: &crate::AppWindow,
    shared_settings: &Rc<RefCell<SettingsManager>>,
) -> Option<(Vec<PathBuf>, PathBuf, ConvertParams, bool)> {
    let convert_all = ui.get_convert_all();
    let file_list = ui.get_file_list();
    let mode = ui.get_selected_output_folder().to_string();

    let paths: Vec<PathBuf> = if convert_all {
        (0..file_list.row_count())
            .filter_map(|i| file_list.row_data(i))
            .map(|t| PathBuf::from(t.path_raw.to_string()))
            .collect()
    } else {
        (0..file_list.row_count())
            .filter_map(|i| file_list.row_data(i))
            .filter(|t| t.is_selected)
            .map(|t| PathBuf::from(t.path_raw.to_string()))
            .collect()
    };

    if paths.is_empty() { return None; }
    
    let persistent_path = ui.get_persistent_output_folder().to_string();

    let output_folder: Option<PathBuf> = if mode == "pick" {
        let picked = rfd::FileDialog::new().pick_folder();
        if let Some(ref folder) = picked {
            let folder_str = folder.to_string_lossy();
            let label = if folder_str.len() > 15 {
                format!("\u{2026}{}", &folder_str[folder_str.len() - 15..])
            } else {
                folder_str.to_string()
            };
            
            ui.set_picked_folder_label(label.into());
            
            // 1. Save full path to our new hidden Slint property
            ui.set_persistent_output_folder(folder_str.to_string().into());
            
            // 2. Select the transformed radio button choice
            ui.set_selected_output_folder("same".into()); 
        }
        picked
    } else if mode == "same" && !persistent_path.is_empty() {
        // Use the saved directory on subsequent clicks
        Some(PathBuf::from(persistent_path))
    } else {
        // Fallback to source directory if they haven't picked a custom one yet
        paths[0].parent().map(|p| p.to_path_buf())
    };
    let folder = output_folder?;
    let settings = shared_settings.borrow();
    let override_files = settings.data.override_files;

    let params = ConvertParams {
        compression: match settings.data.compression.as_str() {
            "lossless" => DngCompression::Lossless,
            "uncompressed" => DngCompression::Uncompressed,
            _ => DngCompression::Lossless,
        },
        crop: match settings.data.crop.as_str() {
            "best" => CropMode::Best,
            "active area" => CropMode::ActiveArea,
            _ => CropMode::None,
        },
        photometric_conversion: DngPhotometricConversion::Original,
        predictor: settings.data.ljpeg92_predictor, 
        embedded: settings.data.embed_raw,
        preview: settings.data.dng_preview,
        thumbnail: settings.data.dng_thumbnail,
        software: "DNGFilm".to_string(),
        artist: if settings.data.artist.is_empty() { None } else { Some(settings.data.artist.clone()) },
        index: settings.data.image_index.parse::<usize>().unwrap_or(0),
        apply_scaling: false,
        keep_mtime: false,
    };

    Some((paths, folder, params, override_files))
}

