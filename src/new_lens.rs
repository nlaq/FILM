use slint::{ComponentHandle, SharedString};
use std::rc::Rc;
use std::cell::RefCell;
use crate::data_manager::DataManager;
use crate::NewLensWindow;

/// Opens the "New Lens" sub-window, wires up all its callbacks,
/// and refreshes the main AppWindow lens list on save.
pub fn show_new_lens_window(
    shared_manager: Rc<RefCell<DataManager>>,
    main_ui_weak: slint::Weak<crate::AppWindow>,
) {
    // ── 1. Instantiate sub-window ────────────────────────────────────────────
    let sub_ui = NewLensWindow::new().expect("Failed to create New Lens Window");
    let sub_ui_handle = sub_ui.as_weak();

    // ── 2. Internal selection state ──────────────────────────────────────────
    let selected_brand        = Rc::new(RefCell::new(String::new()));
    let selected_model        = Rc::new(RefCell::new(String::new()));
    let selected_focal        = Rc::new(RefCell::new(String::new()));
    let selected_max_aperture = Rc::new(RefCell::new(String::new()));
    let selected_min_aperture = Rc::new(RefCell::new(String::new()));

    // ── 3. Populate lists from data_manager ─────────────────────────────────
  // ──  Populate UI Collections initially ───────────────────────────────
    // ── 1. Populate UI Collections initially for NewLensWindow ───────────────────
    {
        let manager = shared_manager.borrow();

        let brands: Vec<SharedString> = manager.data.brands
            .iter().map(|b| SharedString::from(&b.brand_name)).collect();
        sub_ui.set_brand_list(Rc::new(slint::VecModel::from(brands)).into());

        let focals: Vec<SharedString> = manager.data.focal_lengths
            .iter().map(|f| SharedString::from(f)).collect();
        sub_ui.set_focal_list(Rc::new(slint::VecModel::from(focals)).into());

        // NewLensWindow separates max and min aperture lists
        let apertures: Vec<SharedString> = manager.data.apertures
            .iter().map(|a| SharedString::from(format!("f/{}", a))).collect();
            
        let aperture_model: slint::ModelRc<SharedString> = Rc::new(slint::VecModel::from(apertures)).into();
        sub_ui.set_max_aperture_list(aperture_model.clone());
        sub_ui.set_min_aperture_list(aperture_model);
    }    // ── 4. ComboBox change handlers ──────────────────────────────────────────

    sub_ui.on_brand_changed({
        let selected_brand = selected_brand.clone();
        let selected_model = selected_model.clone();
        let shared_manager = shared_manager.clone();
        let win_handle = sub_ui_handle.clone(); // Using your correct weak handle name
        move |val| {
            let Some(win) = win_handle.upgrade() else { return };
            let is_valid = !val.is_empty() && val != "Select...";
            
            *selected_brand.borrow_mut() = if is_valid { val.to_string() } else { String::new() };
            
            // FIX 1: Use the correct method name for NewLensWindow
            win.set_brand_selected(is_valid); 

            // Dynamically load models for the chosen brand
            if is_valid {
                let mgr = shared_manager.borrow();
                if let Some(brand_obj) = mgr.data.brands.iter().find(|b| b.brand_name == val.as_str()) {
                    let brand_models: Vec<SharedString> = brand_obj.models.iter()
                        .map(|m| SharedString::from(m))
                        .collect();
                    win.set_model_list(Rc::new(slint::VecModel::from(brand_models)).into());
                }
            } else {
                // Reset if they choose "Select..." again
                win.set_model_list(Rc::new(slint::VecModel::from(Vec::<SharedString>::new())).into());
                
                // FIX 2: Use the correct method name for NewLensWindow
                win.set_model_selected(false); 
                *selected_model.borrow_mut() = String::new();
            }
        }
    });

    sub_ui.on_model_changed({
        let sub_handle     = sub_ui_handle.clone();
        let selected_model = selected_model.clone();
        move |model| {
            let Some(win) = sub_handle.upgrade() else { return };
            let is_valid = model != "Select..." && !model.is_empty();
            *selected_model.borrow_mut() = if is_valid { model.to_string() } else { String::new() };
            win.set_model_selected(is_valid);
        }
    });

    sub_ui.on_focal_changed({
        let sub_handle     = sub_ui_handle.clone();
        let selected_focal = selected_focal.clone();
        move |focal| {
            let Some(win) = sub_handle.upgrade() else { return };
            let is_valid = focal != "Select..." && !focal.is_empty();
            *selected_focal.borrow_mut() = if is_valid { focal.to_string() } else { String::new() };
            win.set_focal_selected(is_valid);
        }
    });

    sub_ui.on_max_aperture_changed({
        let sub_handle            = sub_ui_handle.clone();
        let selected_max_aperture = selected_max_aperture.clone();
        move |ap| {
            let Some(win) = sub_handle.upgrade() else { return };
            let is_valid = ap != "Select..." && !ap.is_empty();
            let bare = ap.trim_start_matches("f/").to_string();
            *selected_max_aperture.borrow_mut() = if is_valid { bare } else { String::new() };
            win.set_max_aperture_selected(is_valid);
        }
    });

    sub_ui.on_min_aperture_changed({
        let sub_handle            = sub_ui_handle.clone();
        let selected_min_aperture = selected_min_aperture.clone();
        move |ap| {
            let Some(win) = sub_handle.upgrade() else { return };
            let is_valid = ap != "Select..." && !ap.is_empty();
            let bare = ap.trim_start_matches("f/").to_string();
            *selected_min_aperture.borrow_mut() = if is_valid { bare } else { String::new() };
            win.set_min_aperture_selected(is_valid);
        }
    });

    // ── 5. "+" inline-add buttons ────────────────────────────────────────────

    sub_ui.on_add_brand_clicked({
        let sub_handle     = sub_ui_handle.clone();
        let shared_manager = shared_manager.clone();
        move || {
            let Some(win) = sub_handle.upgrade() else { return };
            let clean = win.get_add_brand_input().to_string().trim().to_string();
            if clean.is_empty() { return; }

            let mut manager = shared_manager.borrow_mut();
            if manager.add_brand(clean).is_ok() {
                let brands: Vec<SharedString> = manager.data.brands
                    .iter().map(|b| SharedString::from(&b.brand_name)).collect();
                win.set_brand_list(Rc::new(slint::VecModel::from(brands)).into());
                win.set_add_brand_input("".into());
            }
        }
    });

    sub_ui.on_add_model_clicked({
        let sub_handle     = sub_ui_handle.clone();
        let shared_manager = shared_manager.clone();
        let selected_brand = selected_brand.clone();
        move || {
            let Some(win) = sub_handle.upgrade() else { return };
            let brand_name = selected_brand.borrow().clone();
            let clean      = win.get_add_model_input().to_string().trim().to_string();
            if brand_name.is_empty() || clean.is_empty() { return; }

            let mut manager = shared_manager.borrow_mut();
            if manager.add_model_to_brand(&brand_name, clean).is_ok() {
                if let Some(b) = manager.data.brands.iter().find(|b| b.brand_name == brand_name) {
                    let models: Vec<SharedString> = b.models.iter().map(|m| SharedString::from(m)).collect();
                    win.set_model_list(Rc::new(slint::VecModel::from(models)).into());
                }
                win.set_add_model_input("".into());
            }
        }
    });

    sub_ui.on_add_focal_clicked({
        let sub_handle     = sub_ui_handle.clone();
        let shared_manager = shared_manager.clone();
        move || {
            let Some(win) = sub_handle.upgrade() else { return };
            let raw   = win.get_add_focal_input().to_string().trim().to_string();
            let digits_only: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits_only.is_empty() { return; }

            let focal = format!("{}", digits_only);

            let mut manager = shared_manager.borrow_mut();
            if manager.add_focal_length(focal).is_ok() {
                let focals: Vec<SharedString> = manager.data.focal_lengths
                    .iter().map(|f| SharedString::from(f)).collect();
                win.set_focal_list(Rc::new(slint::VecModel::from(focals)).into());
                win.set_add_focal_input("".into());
            }
        }
    });

    sub_ui.on_add_max_aperture_clicked({
        let sub_handle     = sub_ui_handle.clone();
        let shared_manager = shared_manager.clone();
        move || {
            let Some(win) = sub_handle.upgrade() else { return };
            let raw = win.get_add_max_aperture_input().to_string().trim().to_string();
            let sanitized: String = {
                let mut seen_dot = false;
                raw.chars().filter(|c| {
                    if c.is_ascii_digit() { return true; }
                    if *c == '.' && !seen_dot { seen_dot = true; return true; }
                    false
                }).collect()
            };
            if sanitized.is_empty() { return; }

            let mut manager = shared_manager.borrow_mut();
            if manager.add_aperture(sanitized).is_ok() {
                let apertures: Vec<SharedString> = manager.data.apertures
                    .iter().map(|a| SharedString::from(format!("f/{}", a))).collect();
                
                let aperture_model: slint::ModelRc<SharedString> = Rc::new(slint::VecModel::from(apertures)).into();
                win.set_max_aperture_list(aperture_model.clone());
                win.set_min_aperture_list(aperture_model);
                win.set_add_max_aperture_input("".into());
            }
        }
    });

    sub_ui.on_add_min_aperture_clicked({
        let sub_handle     = sub_ui_handle.clone();
        let shared_manager = shared_manager.clone();
        move || {
            let Some(win) = sub_handle.upgrade() else { return };
            let raw = win.get_add_min_aperture_input().to_string().trim().to_string();
            let sanitized: String = {
                let mut seen_dot = false;
                raw.chars().filter(|c| {
                    if c.is_ascii_digit() { return true; }
                    if *c == '.' && !seen_dot { seen_dot = true; return true; }
                    false
                }).collect()
            };
            if sanitized.is_empty() { return; }

            let mut manager = shared_manager.borrow_mut();
            if manager.add_aperture(sanitized).is_ok() {
                let apertures: Vec<SharedString> = manager.data.apertures
                    .iter().map(|a| SharedString::from(format!("f/{}", a))).collect();
                
                let aperture_model: slint::ModelRc<SharedString> = Rc::new(slint::VecModel::from(apertures)).into();
                win.set_max_aperture_list(aperture_model.clone());
                win.set_min_aperture_list(aperture_model);
                win.set_add_min_aperture_input("".into());
            }
        }
    });

    // ── 6. Save ─────────────────────────────────────────────────────────────

    sub_ui.on_save_clicked({
        let sub_handle        = sub_ui_handle.clone();
        let shared_manager    = shared_manager.clone();
        let main_ui_weak      = main_ui_weak.clone();
        let selected_brand    = selected_brand.clone();
        let selected_model    = selected_model.clone();
        let selected_focal    = selected_focal.clone();
        let selected_max_aperture = selected_max_aperture.clone();
        let selected_min_aperture = selected_min_aperture.clone();
        move || {
            let brand  = selected_brand.borrow().clone();
            let model  = selected_model.borrow().clone();
            let focal  = selected_focal.borrow().clone();
            let max_ap = selected_max_aperture.borrow().clone();
            let min_ap = selected_min_aperture.borrow().clone();

            if brand.is_empty() || model.is_empty() || focal.is_empty() || max_ap.is_empty() || min_ap.is_empty() {
                return;
            }

            let mut manager = shared_manager.borrow_mut();
            if manager.add_lens(brand, model, focal, max_ap, min_ap).is_ok() {
                println!("Lens saved successfully.");

                if let Some(main_win) = main_ui_weak.upgrade() {
                    use crate::AppData;

                    // Stitches values together for display list in main app window (e.g., "Canon RF 50mm f/1.2 - f/16")
let lenses: Vec<SharedString> = manager.data.lenses
    .iter()
    .map(|l| {
        // Appends "mm" cleanly for the user presentation layer only
        let display_focal = if l.focal.ends_with("mm") {
            l.focal.clone()
        } else {
            format!("{}mm", l.focal)
        };

        SharedString::from(format!(
            "{} {} {} f/{} - f/{}", 
            l.brand, l.model, display_focal, l.max_aperture, l.min_aperture
        ))
    })
    .collect();
                        
                    main_win.global::<AppData>()
                        .set_lens_list(Rc::new(slint::VecModel::from(lenses)).into());

                    let brands: Vec<SharedString> = manager.data.brands
                        .iter().map(|b| SharedString::from(&b.brand_name)).collect();
                    main_win.global::<AppData>()
                        .set_brand_list(Rc::new(slint::VecModel::from(brands)).into());
                }
            }

            if let Some(win) = sub_handle.upgrade() {
                let _ = win.hide();
            }
        }
    });

    // ── 7. Cancel ────────────────────────────────────────────────────────────

    sub_ui.on_cancel_clicked({
        let sub_handle = sub_ui_handle.clone();
        move || {
            if let Some(win) = sub_handle.upgrade() {
                let _ = win.hide();
            }
        }
    });

    // ── 8. Show ─────────────────────────────────────────────────────────────
    sub_ui.show().expect("Failed to show New Lens Window");

    std::mem::forget(sub_ui);
}