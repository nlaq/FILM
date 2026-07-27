use std::cell::RefCell;
use std::rc::Rc;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::path::PathBuf;
use crate::{AppWindow, AppData};
use crate::data_manager::DataManager;

pub fn refresh_lens_lists(
    ui: &AppWindow,
    data_manager: &Rc<RefCell<DataManager>>,
    selected_lens_index: &Rc<RefCell<Option<usize>>>, 
) {
    let dm = data_manager.borrow();

    // 1. Formatear y cargar la lista de lentes
    let formatted_lenses: Vec<SharedString> = dm.get_sorted_lenses()
        .iter()
        .map(|l| {
            SharedString::from(format!(
                "{} {} {}mm f/{}",
                l.brand,
                l.model,
                l.focal,
                l.max_aperture
            ))
        })
        .collect();

    ui.global::<AppData>()
        .set_lens_list(
            ModelRc::new(VecModel::from(formatted_lenses))
        );

    // =========================================================================
    // LÓGICA DE FILTRADO CORREGIDA
    // =========================================================================
    let current_index = *selected_lens_index.borrow();
    
    let apertures = match current_index {
        Some(index) => {
            // Si hay una lente seleccionada, extrae únicamente sus aperturas
            let lenses = dm.get_sorted_lenses();
            match lenses.get(index) {
                Some(lens) => dm.get_apertures_for_lens(lens),
                None => Vec::new(),
            }
        }
        None => {
            // Si es el inicio o no hay selección, el combobox de aperturas inicia vacío
            Vec::new()
        }
    };

    let formatted_apertures: Vec<SharedString> = apertures
        .into_iter()
        .map(|a| SharedString::from(a))
        .collect();

    ui.global::<AppData>()
        .set_aperture_list(
            ModelRc::new(VecModel::from(formatted_apertures))
        );
}



/// Called after a conversion batch finishes. Resets the UI's converting
/// state and, if any files failed, shows a single native popup listing them.


pub fn report_conversion_result(
    ui_weak: slint::Weak<AppWindow>,
    failed: Vec<(PathBuf, String)>,
) {
    if let Some(ui) = ui_weak.upgrade() {
        ui.set_is_converting(false);

        if failed.is_empty() {
            // Success: toast only.
            ui.set_toast_text("Conversion complete".into());
            ui.set_toast_visible(true);

            let ui_weak_clone = ui_weak.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(2500), move || {
                if let Some(ui) = ui_weak_clone.upgrade() {
                    ui.set_toast_visible(false);
                }
            });

            return;
        }
    }

    // Failure: no toast, just the detailed popup.
    let list = failed
        .iter()
        .map(|(path, reason)| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            let short_reason = reason
                .split(':')
                .next()
                .unwrap_or(reason)
                .trim();

            format!("{} — {}", name, short_reason)
        })
        .collect::<Vec<_>>()
        .join("\n");

    rfd::MessageDialog::new()
        .set_title("Error")
        .set_description(&format!(
            "Warning! {} file(s) could not be converted:\n\n{}",
            failed.len(),
            list
        ))
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}