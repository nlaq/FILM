use std::cell::RefCell;
use std::rc::{Rc, Weak};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Global};
use crate::models::LensItem;
use crate::DataManager;
use crate::slint_generatedAppWindow::{AppWindow, AppData};


pub struct NewLensController {
    ui: AppWindow,
    pub data_manager: Rc<RefCell<DataManager>>,
    // Internal state tracking fields
    current_brand: RefCell<String>,
    current_model: RefCell<String>,
    current_focal: RefCell<String>,
    current_max_aperture: RefCell<String>,
    current_min_aperture: RefCell<String>,
}

impl NewLensController {
    pub fn new(
        ui: AppWindow,
        data_manager: Rc<RefCell<DataManager>>,
    ) -> Rc<Self> {
    
        let controller = Rc::new(Self {
            ui,
            data_manager,
    
            current_brand: RefCell::new(String::new()),
            current_model: RefCell::new(String::new()),
            current_focal: RefCell::new(String::new()),
            current_max_aperture: RefCell::new(String::new()),
            current_min_aperture: RefCell::new(String::new()),
        });
    
        controller.init_event_handlers();
        controller.refresh_ui_dropdowns();
    
        controller
    }

    pub fn show_window(self: &Rc<Self>) {

        self.reset_state();
        self.refresh_ui_dropdowns();
        self.ui.set_new_lens_box_visible(true);
    }

    fn reset_state(&self) {

        self.current_brand.borrow_mut().clear();
        self.current_model.borrow_mut().clear();
        self.current_focal.borrow_mut().clear();
        self.current_max_aperture.borrow_mut().clear();
        self.current_min_aperture.borrow_mut().clear();

        self.ui.set_new_lens_brand_selected(false);
        self.ui.set_new_lens_model_selected(false);
        self.ui.set_new_lens_focal_selected(false);
        self.ui.set_new_lens_max_aperture_selected(false);
        self.ui.set_new_lens_min_aperture_selected(false);

        self.ui.set_new_lens_add_brand_input(SharedString::default());
        self.ui.set_new_lens_add_model_input(SharedString::default());
        self.ui.set_new_lens_add_focal_input(SharedString::default());
        self.ui.set_new_lens_add_max_aperture_input(SharedString::default());
        self.ui.set_new_lens_add_min_aperture_input(SharedString::default());
    }

    fn build_slint_model(&self, items: Vec<String>) -> ModelRc<SharedString> {
        let shared_strings: Vec<SharedString> =
            items.into_iter()
                .map(SharedString::from)
                .collect();
        ModelRc::from(
            Rc::new(VecModel::from(shared_strings))
        )
    }

    fn refresh_ui_dropdowns(&self) {
        let dm = self.data_manager.borrow();

        let brands = dm.get_sorted_brands();
        self.ui.global::<AppData>().set_brand_list(
            self.build_slint_model(brands)
        );

        // No brand is selected at this point (initial load / dialog reopened),
        // so the model list starts empty rather than depending on how
        // DataManager interprets an empty brand string.
        self.ui.global::<AppData>().set_model_list(
            self.build_slint_model(Vec::new())
        );

        let focals = dm.get_sorted_focal_lengths();
        self.ui.global::<AppData>().set_focal_list(
            self.build_slint_model(focals)
        );

        let apertures = dm.get_sorted_apertures();
        self.ui.global::<AppData>().set_max_aperture_list(
            self.build_slint_model(apertures.clone())
        );
        self.ui.global::<AppData>().set_min_aperture_list(
            self.build_slint_model(apertures)
        );
    }

    fn init_event_handlers(self: &Rc<Self>) {

        // ── Window Management ───────────────────────────────
        let ui_weak = self.ui.as_weak();
        self.ui.on_new_lens_cancel_clicked(move || {
        
            if let Some(ui_handle) = ui_weak.upgrade() { ui_handle.set_new_lens_box_visible(false);}
        });

        // ── Brand Selection ─────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let dm_clone = self.data_manager.clone();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_brand_changed(move |brand| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean_brand = brand.trim();

            if !clean_brand.is_empty()
                && clean_brand != "Select..." {

                *self_clone.current_brand.borrow_mut() =
                    clean_brand.to_string();

                ui_handle.set_new_lens_brand_selected(true);

                let dm = dm_clone.borrow();
                let models =
                    dm.get_sorted_models_for_brand(clean_brand)
                      .unwrap_or_default();

                AppData::get(&ui_handle).set_model_list(
                    self_clone.build_slint_model(models)
                );

            } else {
                ui_handle.set_new_lens_brand_selected(false);
            }
        });

        // ── Model Selection ─────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_model_changed(move |model| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean_model = model.trim();

            if !clean_model.is_empty()
                && clean_model != "Select..." {

                *self_clone.current_model.borrow_mut() =
                    clean_model.to_string();

                ui_handle.set_new_lens_model_selected(true);

            } else {

                ui_handle.set_new_lens_model_selected(false);
            }
        });

        // ── Focal Selection ─────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_focal_changed(move |focal| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean_focal = focal.trim();

            if !clean_focal.is_empty()
                && clean_focal != "Select..." {

                *self_clone.current_focal.borrow_mut() =
                    clean_focal.to_string();

                ui_handle.set_new_lens_focal_selected(true);

            } else {

                ui_handle.set_new_lens_focal_selected(false);
            }
        });

        // ── Max Aperture Selection ───────────────────────────
        let ui_weak = self.ui.as_weak();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_max_aperture_changed(move |aperture| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean_ap = aperture.trim();

            if !clean_ap.is_empty()
                && clean_ap != "Select..." {

                *self_clone.current_max_aperture.borrow_mut() =
                    clean_ap.to_string();

                ui_handle.set_new_lens_max_aperture_selected(true);

            } else {

                ui_handle.set_new_lens_max_aperture_selected(false);
            }
        });

        // ── Min Aperture Selection ───────────────────────────
        let ui_weak = self.ui.as_weak();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_min_aperture_changed(move |aperture| {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let clean_ap = aperture.trim();

            if !clean_ap.is_empty()
                && clean_ap != "Select..." {

                *self_clone.current_min_aperture.borrow_mut() =
                    clean_ap.to_string();

                ui_handle.set_new_lens_min_aperture_selected(true);

            } else {

                ui_handle.set_new_lens_min_aperture_selected(false);
            }
        });

        // ── Add Brand ────────────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let dm_clone = self.data_manager.clone();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_add_brand_clicked(move || {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let input_brand =
                ui_handle.get_new_lens_add_brand_input().to_string();

            let trimmed = input_brand.trim();

            if !trimmed.is_empty() {

                let add_result = dm_clone.borrow_mut().add_brand(trimmed);

                match add_result {
                    Ok(_) => {

                        println!(
                            "Brand added successfully: {}",trimmed
                        );

                        ui_handle.set_new_lens_add_brand_input(
                            SharedString::default()
                        );

                        let brands =
                            dm_clone.borrow()
                                .get_sorted_brands();

                        AppData::get(&ui_handle).set_brand_list(
                            self_clone.build_slint_model(brands)
                        );
                    },
                    Err(e) => {
                        eprintln!("Error adding brand: {}", e);
                    }
                }
            }
        });

        // ── Add Model ────────────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let dm_clone = self.data_manager.clone();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_add_model_clicked(move || {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let input_model =
                ui_handle.get_new_lens_add_model_input().to_string();

            let trimmed = input_model.trim();

            let active_brand =
                self_clone.current_brand.borrow().clone();

            if !trimmed.is_empty()
                && !active_brand.is_empty() {

                let add_result = dm_clone.borrow_mut().add_model(&active_brand, trimmed);

                match add_result {
                    Ok(_) => {

                        println!(
                            "Model added successfully: {} to brand {}",trimmed,active_brand
                        );

                        ui_handle.set_new_lens_add_model_input(
                            SharedString::default()
                        );

                        let models =
                            dm_clone.borrow()
                                .get_sorted_models_for_brand(&active_brand)
                                .unwrap_or_default();

                        AppData::get(&ui_handle).set_model_list(
                            self_clone.build_slint_model(models)
                        );
                    },
                    Err(e) => {
                        eprintln!("Error adding model: {}", e);
                    }
                }
            }
        });

        // ── Add Focal Length ─────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let dm_clone = self.data_manager.clone();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_add_focal_clicked(move || {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let input_focal =
                ui_handle.get_new_lens_add_focal_input().to_string();

            let trimmed = input_focal.trim();

            if !trimmed.is_empty() {

                let add_result = dm_clone.borrow_mut().add_focal_length(trimmed);

                match add_result {
                    Ok(_) => {

                        println!(
                            "Focal added: {}",trimmed
                        );

                        ui_handle.set_new_lens_add_focal_input(
                            SharedString::default()
                        );

                        let focals =
                            dm_clone.borrow()
                                .get_sorted_focal_lengths();

                        AppData::get(&ui_handle).set_focal_list(
                            self_clone.build_slint_model(focals)
                        );
                    },
                    Err(e) => {
                        eprintln!("Error adding focal length: {}", e);
                    }
                }
            }
        });

        // ── Add Max Aperture ─────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let dm_clone = self.data_manager.clone();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_add_max_aperture_clicked(move || {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let input_ap =
                ui_handle.get_new_lens_add_max_aperture_input().to_string();

            let trimmed = input_ap.trim();

            if !trimmed.is_empty() {

                let add_result = dm_clone.borrow_mut().add_aperture(trimmed);

                match add_result {
                    Ok(_) => {

                        println!(
                            "Max Aperture registered: {}",trimmed
                        );

                        ui_handle.set_new_lens_add_max_aperture_input(
                            SharedString::default()
                        );

                        let apertures =
                            dm_clone.borrow()
                                .get_sorted_apertures();

                        AppData::get(&ui_handle).set_max_aperture_list(
                            self_clone.build_slint_model(apertures.clone())
                        );

                        AppData::get(&ui_handle).set_min_aperture_list(
                            self_clone.build_slint_model(apertures)
                        );
                    },
                    Err(e) => {
                        eprintln!("Error adding max aperture: {}", e);
                    }
                }
            }
        });

        // ── Add Min Aperture ─────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let dm_clone = self.data_manager.clone();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_add_min_aperture_clicked(move || {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            let input_ap =
                ui_handle.get_new_lens_add_min_aperture_input().to_string();

            let trimmed = input_ap.trim();

            if !trimmed.is_empty() {

                let add_result = dm_clone.borrow_mut().add_aperture(trimmed);

                match add_result {
                    Ok(_) => {

                        println!(
                            "Min Aperture registered: {}",trimmed
                        );

                        ui_handle.set_new_lens_add_min_aperture_input(
                            SharedString::default()
                        );

                        let apertures =
                            dm_clone.borrow()
                                .get_sorted_apertures();

                        AppData::get(&ui_handle).set_max_aperture_list(
                            self_clone.build_slint_model(apertures.clone())
                        );

                        AppData::get(&ui_handle).set_min_aperture_list(
                            self_clone.build_slint_model(apertures)
                        );
                    },
                    Err(e) => {
                        eprintln!("Error adding min aperture: {}", e);
                    }
                }
            }
        });

        // ── Save Lens ────────────────────────────────────────
        let ui_weak = self.ui.as_weak();
        let dm_clone = self.data_manager.clone();
        let self_weak: Weak<Self> = Rc::downgrade(self);

        self.ui.on_new_lens_save_clicked(move || {
            let (Some(ui_handle), Some(self_clone)) = (ui_weak.upgrade(), self_weak.upgrade()) else { return; };

            println!(
                "Saving lens configuration..."
            );

            let new_lens = LensItem {
                id: 0,
                brand:
                    self_clone.current_brand.borrow().clone(),
                model:
                    self_clone.current_model.borrow().clone(),
                focal:
                    self_clone.current_focal.borrow().clone(),
                max_aperture:
                    self_clone.current_max_aperture.borrow().clone(),
                min_aperture:
                    self_clone.current_min_aperture.borrow().clone(),
            };

            let save_result = dm_clone.borrow_mut().add_lens(new_lens);

            match save_result {
                Ok(_) => {

                    println!(
                        "Lens profile successfully saved."
                    );

                    ui_handle.set_new_lens_box_visible(false);
                },

                Err(e) => {

                    eprintln!(
                        "Error saving lens: {}",e
                    );
                }
            }
        });
    }
}
