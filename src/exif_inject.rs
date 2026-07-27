use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::{error, info, warn};
use slint::Model; // Importante para row_count() y row_data()

use crate::DataManager;

//////////////////////////////////
///////// EXIF METADATA WRITE
//////////////////////////////

pub struct WriteExifParams {
    pub selected_raw_paths: Vec<PathBuf>,
    pub lens_make: String,
    pub lens_model: String,
    pub lens_focal: String,
    pub shooting_aperture: f64,
}

pub type MetadataPayload = (String, String, String, String, String);

/// Extrae y valida los datos seleccionados en la UI. 
/// Retorna `None` si falta algo o si los datos no son válidos.
pub fn extract_params_from_ui(
    ui: &crate::AppWindow,
    data_manager: &RefCell<DataManager>,
    selected_lens_index: &RefCell<Option<usize>>,
    selected_aperture: &RefCell<Option<String>>,
) -> Option<WriteExifParams> {
    // 1. Rutas seleccionadas
    let file_list = ui.get_file_list();
    let selected_raw_paths: Vec<PathBuf> = (0..file_list.row_count())
        .filter_map(|i| file_list.row_data(i))
        .filter(|item| item.is_selected)
        .map(|item| PathBuf::from(item.path_raw.as_str()))
        .collect();

    if selected_raw_paths.is_empty() {
        warn!("[Exif] No RAW files selected.");
        return None;
    }

    // 2. Lente seleccionado
    let lens_index = match *selected_lens_index.borrow() {
        Some(idx) => idx,
        None => {
            warn!("[Exif] No lens selected.");
            return None;
        }
    };

    let lens = {
        let dm = data_manager.borrow();
        let lenses = dm.get_sorted_lenses();
        match lenses.get(lens_index) {
            Some(l) => l.clone(),
            None => {
                warn!(
                    "[Exif] Invalid lens index {lens_index}. Database contains {} lenses.",
                    lenses.len()
                );
                return None;
            }
        }
    };

    // 3. Apertura
    let aperture_str = match selected_aperture.borrow().clone() {
        Some(ap) => ap,
        None => {
            warn!("[Exif] No aperture selected.");
            return None;
        }
    };

    let shooting_aperture: f64 = match aperture_str.parse() {
        Ok(val) => val,
        Err(_) => {
            warn!("[Exif] Invalid aperture value: {}", aperture_str);
            return None;
        }
    };

    Some(WriteExifParams {
        selected_raw_paths,
        lens_make: lens.brand.clone(),
        lens_model: format!("{} {}mm f/{}", lens.model, lens.focal, lens.max_aperture),
        lens_focal: lens.focal.clone(),
        shooting_aperture,
    })
}

/// Función asíncrona que procesa ExifTool y lee los metadatos de los archivos actualizados
pub async fn process_and_fetch_metadata(
    params: WriteExifParams,
    exiftool_bin: Arc<PathBuf>,
) -> Option<MetadataPayload> {
    let bin_clone = exiftool_bin.clone();

    let successful_paths = tokio::task::spawn_blocking(move || {
        write_exif_logic(
            params.selected_raw_paths,
            params.lens_make,
            params.lens_model,
            params.lens_focal,
            params.shooting_aperture,
            &bin_clone,
        )
    })
    .await
    .unwrap_or_default();

    if successful_paths.is_empty() {
        return None;
    }

    let last_path = successful_paths.last()?.clone();
    let bin_clone_2 = exiftool_bin.clone();

    let metadata = tokio::task::spawn_blocking(move || {
        crate::thumbnails_previews::exif_metadata_ui(&bin_clone_2, &last_path)
    })
    .await
    .unwrap_or_else(|_| {
        (
            "Unknown".into(),
            "Unknown".into(),
            "Unknown".into(),
            "Unknown".into(),
            "Unknown".into(),
        )
    });

    Some(metadata)
}

/// Procesa la inyección de metadatos síncronamente para la lista de archivos RAW dados.
/// Retorna un `Vec<PathBuf>` con los archivos actualizados con éxito.
pub fn write_exif_logic(
    selected_raw_paths: Vec<PathBuf>,
    lens_make: String,
    lens_model: String,
    lens_focal: String,
    shooting_aperture: f64,
    exiftool_bin: &Path,
) -> Vec<PathBuf> {
    let mut successful_paths = Vec::new();

    // 1. Construir argumentos reutilizables de ExifTool
    let mut exif_args = Vec::<String>::new();

    // Lens Make
    exif_args.extend(crate::exif_data::inject_lens_make_args(&lens_make));

    // Lens Model
    exif_args.extend(crate::exif_data::inject_lens_model_args(&lens_model));

    // Aperture
    match crate::exif_data::inject_aperture_metadata_args(shooting_aperture) {
        Ok(args) => exif_args.extend(args),
        Err(e) => {
            error!("[Exif] Aperture argument creation failed: {}", e);
            return successful_paths;
        }
    }

    // Focal length
    match crate::exif_data::inject_focal_length_metadata_args(&lens_focal) {
        Ok(args) => exif_args.extend(args),
        Err(e) => {
            error!("[Exif] Focal length argument creation failed: {}", e);
            return successful_paths;
        }
    }

    // 2. Procesar cada archivo seleccionado
    for raw_path in selected_raw_paths {
        info!(
            "[Exif] Writing metadata:\nFile: {}\nLensMake: {}\nLensModel: {}\nAperture: {}",
            raw_path.display(),
            lens_make,
            lens_model,
            shooting_aperture
        );

        if let Err(e) = crate::exif_data::execute_exiftool_write(
            exiftool_bin,
            &raw_path,
            exif_args.clone(),
        ) {
            error!(
                "[Exif] Metadata injection failed for {}: {}",
                raw_path.display(),
                e
            );
        } else {
            info!("[Exif] Metadata successfully written to {}", raw_path.display());
            successful_paths.push(raw_path);
        }
    }

    successful_paths
}