use slint::{ComponentHandle, SharedString};
use std::cell::RefCell;
use std::rc::Rc;
use crate::data_manager::DataManager;

pub fn show_remove_window(
    shared_manager: Rc<RefCell<DataManager>>,
    main_ui_handle: slint::Weak<crate::AppWindow>,
) {
    let sub_ui = crate::RemoveWindow::new().expect("Failed to initialize RemoveWindow");
    let win_handle = sub_ui.as_weak();

    // Create a persistent VectorModel for the models list to preserve reliable UI reactive bindings
    let model_vector = Rc::new(slint::VecModel::<SharedString>::default());
    sub_ui.set_model_list(slint::ModelRc::from(model_vector.clone()));

    // ── 1. Populate UI Collections initially ───────────────────────────────
    {
        let manager = shared_manager.borrow();

        let lenses: Vec<SharedString> = manager.data.lenses
            .iter()
            .map(|l| SharedString::from(format!("{} {} {} f/{} - f/{}", l.brand, l.model, l.focal, l.max_aperture, l.min_aperture)))
            .collect();
        sub_ui.set_lens_list(Rc::new(slint::VecModel::from(lenses)).into());

        let brands: Vec<SharedString> = manager.data.brands
            .iter().map(|b| SharedString::from(&b.brand_name)).collect();
        sub_ui.set_brand_list(Rc::new(slint::VecModel::from(brands)).into());

        // Initialize models list as empty until a brand is selected
        model_vector.set_vec(Vec::new());

        let focals: Vec<SharedString> = manager.data.focal_lengths
            .iter().map(|f| SharedString::from(f)).collect();
        sub_ui.set_focal_list(Rc::new(slint::VecModel::from(focals)).into());

        let apertures: Vec<SharedString> = manager.data.apertures
            .iter().map(|a| SharedString::from(format!("f/{}", a))).collect();
        sub_ui.set_aperture_list(Rc::new(slint::VecModel::from(apertures)).into());
    }

    let selected_lens = Rc::new(RefCell::new(String::new()));
    let selected_brand = Rc::new(RefCell::new(String::new()));
    let selected_model = Rc::new(RefCell::new(String::new()));
    let selected_focal = Rc::new(RefCell::new(String::new()));
    let selected_aperture = Rc::new(RefCell::new(String::new()));

    // ── 2. Handle Combobox State Validations ─────────────────────────────────
    sub_ui.on_lens_changed({
        let selected_lens = selected_lens.clone();
        let win_handle = win_handle.clone();
        move |val| {
            let Some(win) = win_handle.upgrade() else { return };
            let is_valid = !val.is_empty() && val != "Select...";
            *selected_lens.borrow_mut() = if is_valid { val.to_string() } else { String::new() };
            win.set_remove_lens_selected(is_valid);
        }
    });

    sub_ui.on_brand_changed({
        let selected_brand = selected_brand.clone();
        let selected_model = selected_model.clone();
        let shared_manager = shared_manager.clone();
        let win_handle = win_handle.clone();
        let model_vector = model_vector.clone();
        move |val| {
            let Some(win) = win_handle.upgrade() else { return };
            let is_valid = !val.is_empty() && val != "Select...";
            *selected_brand.borrow_mut() = if is_valid { val.to_string() } else { String::new() };
            win.set_remove_brand_selected(is_valid);

            // Dynamically reload models array context matching selected brand criteria
            if is_valid {
                let mgr = shared_manager.borrow();
                if let Some(brand_obj) = mgr.data.brands.iter().find(|b| b.brand_name == val.as_str()) {
                    let mut brand_models: Vec<SharedString> = Vec::new();
                    brand_models.push(SharedString::from("Select..."));
                    brand_models.extend(brand_obj.models.iter().map(|m| SharedString::from(m)));
                    model_vector.set_vec(brand_models);
                }
            } else {
                model_vector.set_vec(Vec::new());
                win.set_remove_model_selected(false);
                *selected_model.borrow_mut() = String::new();
            }
        }
    });

    sub_ui.on_model_changed({
        let selected_model = selected_model.clone();
        let win_handle = win_handle.clone();
        move |val| {
            let Some(win) = win_handle.upgrade() else { return };
            let is_valid = !val.is_empty() && val != "Select...";
            *selected_model.borrow_mut() = if is_valid { val.to_string() } else { String::new() };
            win.set_remove_model_selected(is_valid);
        }
    });

    sub_ui.on_focal_changed({
        let selected_focal = selected_focal.clone();
        let win_handle = win_handle.clone();
        move |val| {
            let Some(win) = win_handle.upgrade() else { return };
            let is_valid = !val.is_empty() && val != "Select...";
            *selected_focal.borrow_mut() = if is_valid { val.to_string() } else { String::new() };
            win.set_remove_focal_selected(is_valid);
        }
    });

    sub_ui.on_aperture_changed({
        let selected_aperture = selected_aperture.clone();
        let win_handle = win_handle.clone();
        move |val| {
            let Some(win) = win_handle.upgrade() else { return };
            let is_valid = !val.is_empty() && val != "Select...";
            *selected_aperture.borrow_mut() = if is_valid { val.trim_start_matches("f/").to_string() } else { String::new() };
            win.set_remove_aperture_selected(is_valid);
        }
    });

    let sync_main_ui = {
        let main_ui_handle = main_ui_handle.clone();
        let shared_manager = shared_manager.clone();
        move || {
            let (lenses, apertures, brands, focals) = {
                let mgr = shared_manager.borrow();
                
                let lenses_strings: Vec<SharedString> = mgr.data.lenses
                    .iter()
                    .map(|l| SharedString::from(format!("{} {} {} f/{} - f/{}", l.brand, l.model, l.focal, l.max_aperture, l.min_aperture)))
                    .collect();

                let apertures_strings: Vec<SharedString> = mgr.data.apertures
                    .iter().map(|a| SharedString::from(format!("f/{}", a))).collect();

                let brands_strings: Vec<SharedString> = mgr.data.brands
                    .iter().map(|b| SharedString::from(&b.brand_name)).collect();

                let focals_strings: Vec<SharedString> = mgr.data.focal_lengths
                    .iter().map(|f| SharedString::from(f)).collect();
                
                (lenses_strings, apertures_strings, brands_strings, focals_strings)
            };

            let _ = slint::invoke_from_event_loop({
                let main_ui_handle = main_ui_handle.clone();
                move || {
                    if let Some(main_ui) = main_ui_handle.upgrade() {
                        let global_data = main_ui.global::<crate::AppData>();
                        global_data.set_lens_list(Rc::new(slint::VecModel::from(lenses)).into());
                        global_data.set_aperture_list(Rc::new(slint::VecModel::from(apertures)).into());
                        global_data.set_brand_list(Rc::new(slint::VecModel::from(brands)).into());
                        global_data.set_focal_list(Rc::new(slint::VecModel::from(focals)).into());
                    }
                }
            });
        }
    };

    // ── 3. Handle Deletion Operations ─────────────────────────────────────────
    sub_ui.on_remove_lens_clicked({
        let shared_manager = shared_manager.clone();
        let selected_lens = selected_lens.clone();
        let win_handle = win_handle.clone();
        let sync_main = sync_main_ui.clone();
        move || {
            let Some(win) = win_handle.upgrade() else { return };
            let lens_str = selected_lens.borrow().clone();
            if lens_str.is_empty() { return; }

            let mut mgr = shared_manager.borrow_mut();
            
            let found_lens = mgr.data.lenses.iter().find(|l| {
                format!("{} {} {} f/{} - f/{}", l.brand, l.model, l.focal, l.max_aperture, l.min_aperture) == lens_str
            }).cloned();

            if let Some(l) = found_lens {
                if mgr.remove_lens(&l.brand, &l.model, &l.focal).is_ok() {
                    let updated: Vec<SharedString> = mgr.data.lenses.iter()
                        .map(|l| SharedString::from(format!("{} {} {} f/{} - f/{}", l.brand, l.model, l.focal, l.max_aperture, l.min_aperture)))
                        .collect();
                    win.set_lens_list(Rc::new(slint::VecModel::from(updated)).into());
                    win.set_remove_lens_selected(false);
                    *selected_lens.borrow_mut() = String::new();
                    
                    drop(mgr); 
                    sync_main();
                }
            }
        }
    });

    sub_ui.on_remove_brand_clicked({
        let shared_manager = shared_manager.clone();
        let selected_brand = selected_brand.clone();
        let win_handle = win_handle.clone();
        let sync_main = sync_main_ui.clone();
        move || {
            let Some(win) = win_handle.upgrade() else { return };
            let brand_str = selected_brand.borrow().clone();
            if brand_str.is_empty() { return; }

            let mut mgr = shared_manager.borrow_mut();
            if mgr.remove_brand(&brand_str).is_ok() {
                let updated: Vec<SharedString> = mgr.data.brands.iter().map(|b| SharedString::from(&b.brand_name)).collect();
                win.set_brand_list(Rc::new(slint::VecModel::from(updated)).into());
                win.set_remove_brand_selected(false);
                *selected_brand.borrow_mut() = String::new();
                
                drop(mgr);
                sync_main();
            }
        }
    });

    sub_ui.on_remove_model_clicked({
        let shared_manager = shared_manager.clone();
        let selected_brand = selected_brand.clone();
        let selected_model = selected_model.clone();
        let win_handle = win_handle.clone();
        let model_vector = model_vector.clone();
        let sync_main = sync_main_ui.clone();
        move || {
            let Some(win) = win_handle.upgrade() else { return };
            let brand_str = selected_brand.borrow().clone();
            let model_str = selected_model.borrow().clone();
            if brand_str.is_empty() || model_str.is_empty() { return; }

            let mut mgr = shared_manager.borrow_mut();
            
            if mgr.remove_model_from_brand(&brand_str, &model_str).is_ok() {
                let mut updated_models: Vec<SharedString> = Vec::new();
                updated_models.push(SharedString::from("Select..."));
                
                if let Some(b) = mgr.data.brands.iter().find(|b| b.brand_name == brand_str) {
                    updated_models.extend(b.models.iter().map(|m| SharedString::from(m)));
                }
                
                model_vector.set_vec(updated_models);
                win.set_remove_model_selected(false);
                *selected_model.borrow_mut() = String::new();

                drop(mgr);
                sync_main();
            }
        }
    });

    sub_ui.on_remove_focal_clicked({
        let shared_manager = shared_manager.clone();
        let selected_focal = selected_focal.clone();
        let win_handle = win_handle.clone();
        let sync_main = sync_main_ui.clone();
        move || {
            let Some(win) = win_handle.upgrade() else { return };
            let raw_focal = selected_focal.borrow().clone();
            if raw_focal.is_empty() { return; }

            let focal_str = raw_focal.trim_end_matches("mm").trim().to_string();

            let mut mgr = shared_manager.borrow_mut();
            if mgr.remove_focal_length(&focal_str).is_ok() {
                let updated: Vec<SharedString> = mgr.data.focal_lengths
                    .iter()
                    .map(|f| SharedString::from(f))
                    .collect();
                win.set_focal_list(Rc::new(slint::VecModel::from(updated)).into());
                win.set_remove_focal_selected(false);
                *selected_focal.borrow_mut() = String::new();

                drop(mgr);
                sync_main();
            }
        }
    });

    sub_ui.on_remove_aperture_clicked({
        let shared_manager = shared_manager.clone();
        let selected_aperture = selected_aperture.clone();
        let win_handle = win_handle.clone();
        let sync_main = sync_main_ui.clone();
        move || {
            let Some(win) = win_handle.upgrade() else { return };
            let ap_str = selected_aperture.borrow().clone();
            if ap_str.is_empty() { return; }

            let mut mgr = shared_manager.borrow_mut();
            if mgr.remove_aperture(&ap_str).is_ok() {
                let updated: Vec<SharedString> = mgr.data.apertures.iter().map(|a| SharedString::from(format!("f/{}", a))).collect();
                win.set_aperture_list(Rc::new(slint::VecModel::from(updated)).into());
                win.set_remove_aperture_selected(false);
                *selected_aperture.borrow_mut() = String::new();
                
                drop(mgr);
                sync_main();
            }
        }
    });

    sub_ui.on_close_clicked(move || {
        let _ = win_handle.upgrade().map(|w| w.hide());
    });

    sub_ui.show().expect("Failed to render RemoveWindow view frame");
}