use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use log::{error, info, warn};
use tokio::sync::Semaphore;

use rawler::dng::{
    convert::ConvertParams,
    CropMode,
    DngCompression,
    DngPhotometricConversion,
};

use crate::data_manager::SettingsManager;
use crate::models::{ConversionRequest};

use dnglab::jobs::raw2dng::Raw2DngJob;
use dnglab::jobs::Job;
use slint::Model;
use crate::exif_data;
use std::sync::Mutex;

// ======================================================
// BUILD CONVERSION REQUEST FROM GUI + SETTINGS
// ======================================================

pub fn conversion_setup(
    ui: &crate::AppWindow,
    settings_manager: &Rc<RefCell<SettingsManager>>,
) -> Option<ConversionRequest> {

    let convert_all = ui.get_convert_all();

    let file_list = ui.get_file_list();

    let input_files: Vec<PathBuf> = if convert_all {

        (0..file_list.row_count())
            .filter_map(|i| file_list.row_data(i))
            .map(|item| PathBuf::from(item.path_raw.to_string()))
            .collect()

    } else {

        (0..file_list.row_count())
            .filter_map(|i| file_list.row_data(i))
            .filter(|item| item.is_selected)
            .map(|item| PathBuf::from(item.path_raw.to_string()))
            .collect()
    };


    if input_files.is_empty() {
        warn!("[Convert] No RAW files selected.");
        return None;
    }

    let output_mode = ui
        .get_selected_output_folder()
        .to_string();

    let output_directory = if output_mode == "pick" {
    
        match rfd::FileDialog::new().pick_folder() {
    
            Some(folder) => {
    
                let label = folder
                    .to_string_lossy()
                    .chars()
                    .rev()
                    .take(15)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
    
                ui.set_picked_folder_label(
                    format!("...{}", label).into()
                );
    
                ui.set_persistent_output_folder(
                    folder.to_string_lossy().to_string().into()
                );
    
                folder
            }
    
            None => {
                warn!("[Convert] Output folder selection cancelled.");
                return None;
            }
        }
    
    } else {
        // User selected "Same as input"
        // If a previous pick-folder selection exists, use it.
        let stored_folder =
            ui.get_persistent_output_folder()
                .to_string();
    
        if !stored_folder.trim().is_empty() {
    
            let folder = PathBuf::from(stored_folder);
    
            if folder.exists() {
                info!(
                    "[Convert] Using remembered output folder: {}",
                    folder.display()
                );
    
                folder
    
            } else {
    
                warn!(
                    "[Convert] Stored output folder no longer exists, falling back to input folder."
                );
    
                match input_files[0].parent() {
    
                    Some(parent) => parent.to_path_buf(),
    
                    None => {
                        error!("[Convert] Unable to determine source folder.");
                        return None;
                    }
                }
            }
    
        } else {
    
            match input_files[0].parent() {
    
                Some(parent) => parent.to_path_buf(),
    
                None => {
                    error!("[Convert] Unable to determine source folder.");
                    return None;
                }
            }
        }
    };

    let settings = settings_manager.borrow()
        .get_settings();

    Some(ConversionRequest {

        input_files,

        output_directory,

        //output_mode,

        artist: settings.artist,

        compression: settings.compression,

        crop: settings.crop,

        dng_preview: settings.dng_preview,

        dng_thumbnail: settings.dng_thumbnail,

        embed_raw: settings.embed_raw,

        override_files: settings.override_files,

        image_index: settings.image_index,

        ljpeg92_predictor: settings.ljpeg92_predictor,
    })
}

// ======================================================
// CREATE RAWLER CONVERT PARAMETERS
// ======================================================

fn build_convert_params(
    request: &ConversionRequest
) -> ConvertParams {

    ConvertParams {

        compression:
            match request.compression.as_str() {

                "uncompressed" =>
                    DngCompression::Uncompressed,

                _ =>
                    DngCompression::Lossless,
            },

        crop:
            match request.crop.as_str() {

                "best" =>
                    CropMode::Best,

                "active area" =>
                    CropMode::ActiveArea,

                _ =>
                    CropMode::None,
            },

        photometric_conversion:
            DngPhotometricConversion::Original,

        predictor:
            request.ljpeg92_predictor,

        embedded:
            request.embed_raw,

        preview:
            request.dng_preview,

        thumbnail:
            request.dng_thumbnail,

        software:
            "DNGFilm".to_string(),

        artist:
            if request.artist.trim().is_empty() {
                None
            } else {
                Some(request.artist.clone())
            },

        index:
            request.image_index
                .parse::<usize>()
                .unwrap_or(0),

        apply_scaling:
            false,

        keep_mtime:
            false,
    }
}

// ======================================================
// CONVERSION CONTROLLER
// ======================================================

#[derive(Clone)]
pub struct ConversionController {

    semaphore: Arc<Semaphore>,
}

impl ConversionController {

    pub fn new() -> Self {

        Self {

            // maximum four RAW conversions simultaneously
            semaphore:
                Arc::new(
                    Semaphore::new(4)
                ),
        }
    }

    pub async fn convert(
        &self,
        request: ConversionRequest,
    ) -> Vec<(PathBuf, String)> {
    
        info!(
            "[Convert] Starting conversion of {} files.",
            request.input_files.len()
        );
        
        let exiftool =
            match exif_data::ensure_embedded_exiftool() {
                Ok(path) => path,
                Err(e) => {
                    error!("[ExifTool] Initialization failed: {}", e);
                    return Vec::new();
                }
            };
       
        let params = build_convert_params(&request);
      
        let mut tasks = Vec::new();
        let failed: Arc<Mutex<Vec<(PathBuf, String)>>> = Arc::new(Mutex::new(Vec::new()));
    
        for input in request.input_files {
        
            let semaphore = self.semaphore.clone();
            let output_directory = request.output_directory.clone();
            let params = params.clone();
            let overwrite = request.override_files;
            let exiftool = exiftool.clone();
            let failed = failed.clone();
    
            let task = tokio::spawn(async move {

                let _permit =
                    semaphore
                    .acquire_owned()
                    .await
                    .expect("Semaphore closed");
            
                let mut output = output_directory;
            
                let stem = match input.file_stem() {
                    Some(v) => v.to_string_lossy(),
                    None => {
                        error!(
                            "[Convert] Invalid filename {:?}",
                            input
                        );
                        failed.lock().unwrap().push((
                            input.clone(),
                            "Invalid filename (no file stem)".to_string(),
                        ));
                        return;
                    }
                };
            
                output.push(
                    format!("{}.dng", stem)
                );
            
                // =============================================================
                // PASO PREVENTIVO: Extraer el buffer binario original de alta calidad
                // =============================================================
                let preview_bytes = match crate::exif_data::extract_preview_image(&exiftool, &input) {
                    Ok(bytes) => Some(bytes),
                    Err(e) => {
                        warn!(
                            "[ExifTool] Could not extract high-res preview from source: {}. Proceeding without it.", 
                            e
                        );
                        None
                    }
                };
            
                let job = Raw2DngJob {
                    input: input.clone(),
                    output: output.clone(),
                    replace: overwrite,
                    params,
                };
            
                // ==================================
                // STEP 1: RAW -> DNG
                // ==================================
                let result = job.execute().await;
            
                match result.error {
                    Some(err) => {
                        error!(
                            "[Convert] Failed {:?}: {}",
                            input,
                            err
                        );
                        failed.lock().unwrap().push((input.clone(), err.to_string()));
                        return;
                    }
                    None => {
                        info!(
                            "[Convert] Finished conversion {:?}",
                            input
                        );
                    }
                }
            
                // ==================================
                // STEP 2: COPY TAGS RAW -> DNG (TEXT ONLY)
                // ==================================
                let mut exif_args = Vec::<String>::new();
                exif_args.extend(
                    crate::exif_data::inject_all_tags_args(
                        &input
                    )
                );
            
                if let Err(e) =
                    crate::exif_data::execute_exiftool_write(
                        &exiftool,
                        &output,
                        exif_args
                    )
                {
                    error!(
                        "[Convert] Metadata injection failed for {:?}: {}",
                        output,
                        e
                    );
                }
                else {
                    info!(
                        "[Convert] Metadata successfully injected into {:?}",
                        output
                    );
                }
            
                // =============================================================
                // STEP 3: FORCE INJECT STANDALONE PREVIEW BYTES 
                // =============================================================
                if let Some(bytes) = preview_bytes {
                    info!("[ExifTool] Streaming pristine high-res JPEG bytes from memory into target DNG...");
            
                    if let Err(e) = crate::exif_data::inject_extracted_preview_bytes(
                        &exiftool, 
                        &output, 
                        &bytes
                    ) {
                        error!(
                            "[Convert] Binary preview injection failed for {:?}: {}", 
                            output, 
                            e
                        );
                    } else {
                        info!(
                            "[Convert] Success! High-res preview perfectly locked into {:?}", 
                            output
                        );
                    }
                }
            });
    
            tasks.push(task);
        }
    
        for task in tasks {
            if let Err(e) = task.await {
                error!("[Convert] A conversion task panicked or was cancelled: {}", e);
            }
        }
    
        Arc::try_unwrap(failed)
            .map(|m| m.into_inner().unwrap())
            .unwrap_or_else(|arc| arc.lock().unwrap().clone())
    }
}

