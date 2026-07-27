use std::rc::{Rc, Weak};
use std::cell::RefCell;
use slint::{ModelRc, VecModel, SharedString, ComponentHandle};
use slint::Global;
use crate::DataManager; 
use crate::slint_generatedAppWindow::{AppWindow, AppData};

pub struct RemoveLensController {
    // 2. Store the main UI handle instead
    ui: AppWindow,
    pub data_manager: Rc<RefCell<DataManager>>,
    current_brand: RefCell<String>,
    current_model: RefCell<String>,
    current_lens_id: RefCell<Option<u64>>,
    current_focal: RefCell<String>,
    current_aperture: RefCell<String>,
}

impl RemoveLensController {
    // 3. Pass the cloned AppWindow handle into the new constructor
    pub fn new(ui: AppWindow, data_manager: Rc<RefCell<DataManager>>) -> Rc<Self> {
        let controller = Rc::new(Self {
            ui,
            data_manager,
            current_brand: RefCell::new(String::new()),
            current_model: RefCell::new(String::new()),
            current_lens_id: RefCell::new(None),
            current_focal: RefCell::new(String::new()),
            current_aperture: RefCell::new(String::new()),
        });

        controller.init_event_handlers();
        controller.refresh_ui_dropdowns();
        
        controller
    }

    // 4. This now toggles the visibility state property instead of opening an operating system window
    pub fn show_window(self: &Rc<Self>) {
        self.reset_state();
        self.refresh_ui_dropdowns();
        self.ui.set_remove_box_visible(true);
    }

    fn reset_state(&self) {
        self.current_brand.borrow_mut().clear();
        self.current_model.borrow_mut().clear();
        self.current_focal.borrow_mut().clear();
        self.current_aperture.borrow_mut().clear();
        *self.current_lens_id.borrow_mut() = None;

        self.ui.set_remove_lens_selected(false);
        self.ui.set_remove_brand_selected(false);
        self.ui.set_remove_model_selected(false);
        self.ui.set_remove_focal_selected(false);
        self.ui.set_remove_aperture_selected(false);
    }

    fn build_slint_model(&self, items: Vec<String>) -> ModelRc<SharedString> {
        let shared_strings: Vec<SharedString> = items.into_iter().map(SharedString::from).collect();
        ModelRc::from(Rc::new(VecModel::from(shared_strings)))
    }

    pub fn refresh_ui_dropdowns(&self) {
        let (lenses, brands, focals, apertures, models) = {
            let dm = self.data_manager.borrow();
            
            let lenses: Vec<SharedString> = dm.get_sorted_lenses().iter()
                .map(|l| SharedString::from(format!("{} {} {}mm f/{}", l.brand, l.model, l.focal, l.max_aperture)))
                .collect();

            let brands = dm.get_sorted_brands();
            let focals = dm.get_sorted_focal_lengths();
            let apertures = dm.get_sorted_apertures();

            let active_brand = self.current_brand.borrow().clone();
            let models = if active_brand.is_empty() {
                Vec::new()
            } else {
                dm.get_sorted_models_for_brand(&active_brand).unwrap_or_default()
            };

            (lenses, brands, focals, apertures, models)
        }; 

        // Modifies the exposed properties directly on the shared main window instance
        let app_data = AppData::get(&self.ui);
        AppData::get(&self.ui).set_lens_list(ModelRc::from(Rc::new(VecModel::from(lenses))));
        app_data.set_brand_list(self.build_slint_model(brands));
        app_data.set_focal_list(self.build_slint_model(focals));
        app_data.set_aperture_list(self.build_slint_model(apertures));
        app_data.set_model_list(self.build_slint_model(models));
    }

    fn init_event_handlers(self: &Rc<Self>) {
        // ── Window Routing ──────────────────────────────────────────────────
        let ui_weak = self.ui.as_weak();
        self.ui.on_close_clicked(move || {
            if let Some(ui_handle) = ui_weak.upgrade() {
                ui_handle.set_remove_box_visible(false);
            }
        });

        // ── Selection State Observers ───────────────────────────────────────
        let self_weak: Weak<Self> = Rc::downgrade(self);
        let ui_weak = self.ui.as_weak();

        // 1. Lens Change Listener (Uses prefixed callback name, but un-prefixed setter)
        self.ui.on_remove_box_lens_changed(move |val: slint::SharedString| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean = val.trim();
            let is_valid = !clean.is_empty() && clean != "Select...";

            let selected_id = if is_valid {
                let dm = self_clone.data_manager.borrow();

                dm.get_sorted_lenses()
                    .iter()
                    .find(|l| {
                        format!(
                            "{} {} {}mm f/{}",
                            l.brand,
                            l.model,
                            l.focal,
                            l.max_aperture
                        ) == clean
                    })
                    .map(|l| l.id)

            } else {
                None
            };

            *self_clone.current_lens_id.borrow_mut() = selected_id;

            // FIXED: Removed the "_box_" prefix from setter to match Main.slint property definition
            ui_handle.set_remove_lens_selected(selected_id.is_some());
        });

        let self_weak: Weak<Self> = Rc::downgrade(self);
        let ui_weak = self.ui.as_weak();
        let dm_clone = self.data_manager.clone();

        // 2. Brand Change Listener (Fixed callback name & un-prefixed setters)
        self.ui.on_remove_box_brand_changed(move |val: slint::SharedString| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean = val.trim();
            let is_valid = !clean.is_empty() && clean != "Select...";

            *self_clone.current_brand.borrow_mut() = if is_valid { clean.to_string() } else { String::new() };
            *self_clone.current_model.borrow_mut() = String::new();

            // FIXED: Removed "_box_" prefix from setters
            ui_handle.set_remove_brand_selected(is_valid);
            ui_handle.set_remove_model_selected(false);

            let models = if is_valid {
                dm_clone.borrow().get_sorted_models_for_brand(clean).unwrap_or_default()
            } else {
                Vec::new()
            };

            AppData::get(&ui_handle).set_model_list(
                self_clone.build_slint_model(models)
            );
        });

        let self_weak: Weak<Self> = Rc::downgrade(self);
        let ui_weak = self.ui.as_weak();

        // 3. Model Change Listener (Fixed callback name & un-prefixed setter)
        self.ui.on_remove_box_model_changed(move |val: slint::SharedString| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean = val.trim();
            let is_valid = !clean.is_empty() && clean != "Select...";
            *self_clone.current_model.borrow_mut() = if is_valid { clean.to_string() } else { String::new() };

            // FIXED: Removed "_box_" prefix from setter
            ui_handle.set_remove_model_selected(is_valid);
        });

        let self_weak: Weak<Self> = Rc::downgrade(self);
        let ui_weak = self.ui.as_weak();

        // 4. Focal Length Change Listener (Fixed callback name & un-prefixed setter)
        self.ui.on_remove_box_focal_changed(move |val: slint::SharedString| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean = val.trim();
            let is_valid = !clean.is_empty() && clean != "Select...";
            *self_clone.current_focal.borrow_mut() = if is_valid { clean.to_string() } else { String::new() };

            // FIXED: Removed "_box_" prefix from setter
            ui_handle.set_remove_focal_selected(is_valid);
        });

        let self_weak: Weak<Self> = Rc::downgrade(self);
        let ui_weak = self.ui.as_weak();

        // 5. Max Aperture Change Listener (Fixed callback name & un-prefixed setter)
        self.ui.on_remove_box_aperture_changed(move |val: slint::SharedString| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean = val.trim();
            let is_valid = !clean.is_empty() && clean != "Select...";
            *self_clone.current_aperture.borrow_mut() = if is_valid { clean.to_string() } else { String::new() };

            // FIXED: Removed "_box_" prefix from setter
            ui_handle.set_remove_aperture_selected(is_valid);
        });

        // ── Core Mutators / Click Triggers ──────────────────────────────────
        let self_weak: Weak<Self> = Rc::downgrade(self);
        self.ui.on_remove_box_lens_clicked(move || {
            let Some(self_clone) = self_weak.upgrade() else { return; };

            let id = *self_clone.current_lens_id.borrow();

            if let Some(id) = id {
                let result = {
                    let mut dm = self_clone.data_manager.borrow_mut();
                    dm.remove_lens(id)
                };

                match result {
                    Ok(_) => {
                        *self_clone.current_lens_id.borrow_mut() = None;
                        self_clone.refresh_ui_dropdowns();
                    }
                    Err(e) => {
                        eprintln!("Error removing lens: {}", e);
                    }
                }
            }
        });

        let self_weak: Weak<Self> = Rc::downgrade(self);
        self.ui.on_remove_box_brand_clicked(move || {
            let Some(self_clone) = self_weak.upgrade() else { return; };

            let brand = self_clone.current_brand.borrow().clone();

            if brand.is_empty() {
                return;
            }

            let result = {
                let mut dm = self_clone.data_manager.borrow_mut();
                dm.remove_brand(&brand)
            };

            match result {
                Ok(_) => {
                    *self_clone.current_brand.borrow_mut() = String::new();
                    self_clone.refresh_ui_dropdowns();
                }
                Err(e) => {
                    eprintln!("Error removing brand: {}", e);
                }
            }
        });

        // Al final de init_event_handlers() en src/remove_lens.rs

        // 3. Callback para eliminar un Modelo
        let self_weak: Weak<Self> = Rc::downgrade(self);
        self.ui.on_remove_box_model_clicked(move || {
            let Some(self_clone) = self_weak.upgrade() else { return; };

            let brand = self_clone.current_brand.borrow().clone();
            let model = self_clone.current_model.borrow().clone();

            if brand.is_empty() || model.is_empty() {
                return;
            }

            let result = {
                let mut dm = self_clone.data_manager.borrow_mut();
                dm.remove_model(&brand, &model) // <-- Usando el método del DataManager
            };

            match result {
                Ok(_) => {
                    *self_clone.current_model.borrow_mut() = String::new();
                    self_clone.refresh_ui_dropdowns();
                }
                Err(e) => {
                    eprintln!("Error removing model: {}", e);
                }
            }
        });

        // 4. Callback para eliminar una Distancia Focal
        let self_weak: Weak<Self> = Rc::downgrade(self);
        self.ui.on_remove_box_focal_clicked(move || {
            let Some(self_clone) = self_weak.upgrade() else { return; };

            let focal = self_clone.current_focal.borrow().clone();

            if focal.is_empty() {
                return;
            }

            let result = {
                let mut dm = self_clone.data_manager.borrow_mut();
                dm.remove_focal_length(&focal) // <-- Usando el método del DataManager
            };

            match result {
                Ok(_) => {
                    *self_clone.current_focal.borrow_mut() = String::new();
                    self_clone.refresh_ui_dropdowns();
                }
                Err(e) => {
                    eprintln!("Error removing focal length: {}", e);
                }
            }
        });

        // 5. Callback para eliminar una Apertura
        let self_weak: Weak<Self> = Rc::downgrade(self);
        self.ui.on_remove_box_aperture_clicked(move || {
            let Some(self_clone) = self_weak.upgrade() else { return; };

            let aperture = self_clone.current_aperture.borrow().clone();

            if aperture.is_empty() {
                return;
            }

            let result = {
                let mut dm = self_clone.data_manager.borrow_mut();
                dm.remove_aperture(&aperture) // <-- Usando el método del DataManager
            };

            match result {
                Ok(_) => {
                    *self_clone.current_aperture.borrow_mut() = String::new();
                    self_clone.refresh_ui_dropdowns();
                }
                Err(e) => {
                    eprintln!("Error removing aperture: {}", e);
                }
            }
        });
    }
}
