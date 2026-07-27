use crate::models::AppSettings;
use crate::data_manager::SettingsManager; 
use slint::{SharedString, Global};
use crate::{AppWindow, DngSettings};
use std::cell::RefCell;
use std::rc::Rc;

pub struct SettingsController {
    ui: AppWindow,
    settings_manager: Rc<RefCell<SettingsManager>>,
}

impl SettingsController {
    /// Creates a new controller instance
    pub fn new(
        ui: AppWindow,
        settings_manager: Rc<RefCell<SettingsManager>>
    ) -> Rc<Self> {
        let controller = Rc::new(Self {
            ui,
            settings_manager,
        });
    
        let weak_controller = Rc::downgrade(&controller);
    
        // Clone the weak pointer for the first closure
        let weak_save = weak_controller.clone();
        controller.ui.on_settings_save_clicked(move || {
            if let Some(c) = weak_save.upgrade() {
                c.save();
            }
        });
    
        // Move the original weak pointer into the second closure
        controller.ui.on_settings_cancel_clicked(move || {
            if let Some(c) = weak_controller.upgrade() {
                c.cancel();
            }
        });
    
        controller
    }

    /// Spawns the separate SettingsWindow on demand and attaches its lifecycle 
    pub fn show_window(&self) {
        // Keeps the borrow scope strictly confined to this block
        {
            let manager = self.settings_manager.borrow();
            self.hydrate_ui_properties(&self.ui, &manager);
        }
        
        self.ui.set_settings_box_visible(true);
    }

    /// Reads configurations through SettingsManager and maps them directly into Slint's global properties
    fn hydrate_ui_properties(&self, ui: &AppWindow, settings_manager: &SettingsManager) {
        let settings = settings_manager.get_settings(); 
        let dng_settings = DngSettings::get(ui);

        dng_settings.set_artist(SharedString::from(&settings.artist));
        dng_settings.set_compression(SharedString::from(&settings.compression));
        dng_settings.set_crop(SharedString::from(&settings.crop));
        dng_settings.set_dng_preview(settings.dng_preview);
        dng_settings.set_dng_thumbnail(settings.dng_thumbnail);
        dng_settings.set_embed_raw(settings.embed_raw);
        dng_settings.set_override_files(settings.override_files);
        dng_settings.set_image_index(SharedString::from(&settings.image_index));

        // Using .max(0) to safely prevent underflows if your data changes later
        let slint_index = (settings.ljpeg92_predictor as i32 - 1).max(0);
        dng_settings.set_ljpeg92_predictor(slint_index);
    }

    /// Scrapes current state of UI elements and creates a clean AppSettings struct
    fn extract_from_ui(ui: &AppWindow) -> AppSettings {
        let dng_settings = DngSettings::get(ui);
        let raw_predictor = (dng_settings.get_ljpeg92_predictor() + 1) as u8;

        AppSettings {
            artist: dng_settings.get_artist().to_string(),
            compression: dng_settings.get_compression().to_string(),
            crop: dng_settings.get_crop().to_string(),
            dng_preview: dng_settings.get_dng_preview(),
            dng_thumbnail: dng_settings.get_dng_thumbnail(),
            embed_raw: dng_settings.get_embed_raw(),
            override_files: dng_settings.get_override_files(),
            image_index: dng_settings.get_image_index().to_string(),
            ljpeg92_predictor: raw_predictor,
        }
    }
    
    pub fn save(&self) {
        let updated_settings = Self::extract_from_ui(&self.ui);
        
        // Try to borrow mutably to prevent random runtime panics
        if let Ok(mut sm) = self.settings_manager.try_borrow_mut() {
            sm.settings = updated_settings;
            
            match sm.save_to_disk() {
                Ok(_) => {
                    println!("Configurations successfully updated in settings.json");
                    self.ui.set_settings_box_visible(false);
                }
                Err(e) => {
                    eprintln!("Critical: Failed to save configuration data: {}", e);
                    // TODO: Trigger a UI alert notification here so the user knows it failed!
                }
            }
        } else {
            eprintln!("Error: Settings manager is currently borrowed elsewhere.");
        }
    }
    
    pub fn cancel(&self) {
        println!("Settings modifications canceled by user.");
        self.ui.set_settings_box_visible(false);
    }
}