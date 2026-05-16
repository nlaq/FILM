// Copyright (C) 2026 nlq. This software is distributed under the GNU GPL v3 license.

use rfd::FileDialog;
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;


use dnglab::jobs::raw2dng::Raw2DngJob;
use dnglab::jobs::Job; 
use rawler::dng::convert::ConvertParams;
use rawler::dng::{DngCompression, CropMode};

use std::fs;
use std::os::unix::fs::PermissionsExt;

slint::include_modules!();


const EXIFTOOL_ZIP: &[u8] = include_bytes!("../resources/exiftool.zip");

// -----------------------------------------------------------------------------
// DNG REPAIR FUNCTION
// -----------------------------------------------------------------------------
fn repair_with_embedded_exiftool(raw_path: &PathBuf, dng_path: &PathBuf) -> Result<String, String> {
    let temp_dir = std::env::temp_dir().join("dnglab_engine_v1");
    let exiftool_executable = temp_dir.join("exiftool");

    // TEMP DIR
    if !exiftool_executable.exists() {
        let _ = fs::create_dir_all(&temp_dir);
        let reader = std::io::Cursor::new(EXIFTOOL_ZIP);
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let outpath = temp_dir.join(file.name());

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath).ok();
            } else {
                if let Some(p) = outpath.parent() { fs::create_dir_all(p).ok(); }
                
                let mut outfile = fs::File::create(&outpath).unwrap();
               
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
                
                // EXEC Permisions
                let mut perms = fs::metadata(&outpath).unwrap().permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&outpath, perms);
            }
        }
    }

    // PERL
    let output = std::process::Command::new("perl")
        .env("PERL5LIB", temp_dir.join("lib"))
        .arg(exiftool_executable)
        .arg("-tagsfromfile")
        .arg(raw_path)
        .arg("-ThumbnailImage")
        .arg("-PreviewImage")
        .arg("-overwrite_original")
        .arg(dng_path)
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                Ok(format!("Fixed: {:?}", dng_path.file_name().unwrap()))
            } else {
                Err(String::from_utf8_lossy(&out.stderr).to_string())
            }
        }
        Err(e) => Err(format!("Error command not executed: {}", e)),
    }
}


////------/////


// -----------------------------------------------------------------------------
// CONVERT
// -----------------------------------------------------------------------------


async fn run_conversion(
    input: PathBuf, 
    output_dir: PathBuf, 
    comp_str: String, 
    crop_str: String, 
    embed_raw: bool, 
    overwrite: bool
) {
    let params = ConvertParams {
        compression: match comp_str.as_str() {
            "lossless" => DngCompression::Lossless, 
            "uncompressed" => DngCompression::Uncompressed,
            _ => DngCompression::Lossless,
        },
        crop: match crop_str.as_str() {
            "best" => CropMode::Best,
            "active area" => CropMode::ActiveArea,
            _ => CropMode::None,
        },
        predictor: 1, // 1 = no predictor (u8 esperado)
        embedded: embed_raw,
        preview: true,
        thumbnail: true,
        software: "DNGFilm".to_string(),
        ..Default::default() 
    };

    let mut output = output_dir;
    if let Some(stem) = input.file_stem() {
        output.push(format!("{}.dng", stem.to_string_lossy()));
    }

    let job = Raw2DngJob {
        input,
        output,
        replace: overwrite,
        params,
    };

    // Tokio runtime needed
    let result = job.execute().await;
    
    if let Some(err) = result.error {
        eprintln!("Error en {:?}: {}", result.job.input.display(), err);
    } else {
        println!("Éxito: {:?}", result.job.input.display());
    }
}

// -----------------------------------------------------------------------------
// HELPERS Y MAIN
// -----------------------------------------------------------------------------

fn check_and_enable_convert(ui: &MainWindow, inputs: &Vec<PathBuf>, output: &PathBuf) {
    let has_inputs = !inputs.is_empty();
    let has_output = !output.as_os_str().is_empty();
    ui.set_convert_true(has_inputs && has_output);
}

// RUNTIME 
#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    let main_window_weak = main_window.as_weak();

    let input_paths = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
    let output_path = Rc::new(RefCell::new(PathBuf::new()));

    // Handler: Files select
    let input_paths_f = Rc::clone(&input_paths);
    let output_path_f = Rc::clone(&output_path);
    let ui_f = main_window_weak.clone();
    main_window.on_input_files(move || {
        let ui = ui_f.upgrade().unwrap();
        if let Some(paths) = FileDialog::new()
            .add_filter("Raw", &[ "ari",
      "cr3",
      "cr2",
      "crw",
      "erf",
      "raf",
      "3fr",
      "kdc",
      "dcs",
      "dcr",
      "iiq",
      "mos",
      "mef",
      "mrw",
      "nef",
      "nrw",
      "orf",
      "rw2",
      "pef",
      "srw",
      "arw",
      "srf",
      "sr2", "dng"])
            .pick_files() 
        {
            *input_paths_f.borrow_mut() = paths.clone();
            ui.set_input_path(format!("{} archivo(s) seleccionados", paths.len()).into());
            check_and_enable_convert(&ui, &input_paths_f.borrow(), &output_path_f.borrow());
        }
    });

    // Handler: Dir Select
    let input_paths_o = Rc::clone(&input_paths);
    let output_path_o = Rc::clone(&output_path);
    let ui_o = main_window_weak.clone();
    
    main_window.on_output_folder(move || {
        let ui = ui_o.upgrade().unwrap();
        if let Some(path) = FileDialog::new().pick_folder() {
            *output_path_o.borrow_mut() = path.clone();
            
            let path_str = path.to_string_lossy().to_string();
            let max_chars = 25; // Ajustar según el ancho de la UI

            // Lógica de recorte: si es muy largo, toma el final
            let display_text = if path_str.chars().count() > max_chars {
                let suffix: String = path_str.chars()
                    .rev()
                    .take(max_chars - 3)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                format!("...{}", suffix)
            } else {
                path_str
            };

            ui.set_output_path(display_text.into());
            check_and_enable_convert(&ui, &input_paths_o.borrow(), &output_path_o.borrow());
        }
    });

    // Handler: Convert
    let input_paths_c = Rc::clone(&input_paths);
    let output_path_c = Rc::clone(&output_path);
    let ui_c = main_window_weak.clone();
    
    main_window.on_convert_pressed(move || {
        let ui = ui_c.upgrade().unwrap();
        
        ui.set_convert_true(false); //Button disabled
        
        let inputs = input_paths_c.borrow().clone();
        let out_dir = output_path_c.borrow().clone();
        
        let comp = ui.get_compression().to_string();
        let crop = ui.get_crop().to_string();
        let embed = ui.get_embeded();
        let ovr = ui.get_override_1();

        // Usamos tokio::spawn for the interface not to block 
        let ui_handle = ui_c.clone(); 
        
        
        
        tokio::spawn(async move {
            for file in inputs {                
                
                //////-----//////
                let file_for_conv = file.clone();
                let file_for_repair = file.clone();
                
                
                
                run_conversion(
                    file, 
                    out_dir.clone(), 
                    comp.clone(), 
                    crop.clone(), 
                    embed, 
                    ovr
                ).await;  
                
                
                /////// dng file fix call
                // 1. Convert
                run_conversion(file_for_conv, out_dir.clone(), comp.clone(), crop.clone(), embed, ovr).await;                      
                
                // 2. Repair
                let dng_name = format!("{}.dng", file_for_repair.file_stem().unwrap().to_string_lossy());
                let dng_path = out_dir.join(dng_name);
                
                // Result
                match repair_with_embedded_exiftool(&file_for_repair, &dng_path) {
                    Ok(msg) => println!("✅ {}", msg),
                    Err(e) => eprintln!("❌ {}", e),
                }
                            
                                    
                               
            }           
            
             // 2. Button reactivation
            let ui_handle_final = ui_handle.clone();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui_ready) = ui_handle_final.upgrade() {
                    // 2. Reactivamos el botón
                    ui_ready.set_convert_true(true);
                }
            });
            

            println!("Process complete.");
        });
    });


    main_window.run()
}



