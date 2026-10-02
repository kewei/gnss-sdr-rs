use eframe::egui;
use crate::visualization::navigation_gui::NavigationGui;
use crate::visualization::acquisition_gui::AcquisitionGui;
use crate::visualization::tracking_gui::TrackingGui;

pub struct GnssSdrRsGui {
    pub acquisition_gui: AcquisitionGui,
    // pub tracking_gui: TrackingGui,
    // pub navigation_gui: NavigationGui,
}

impl GnssSdrRsGui {
    pub fn new(acq: AcquisitionGui) -> Self {
        Self {
            acquisition_gui: acq,
            // tracking_gui: TrackingGui::new(),
            // navigation_gui: NavigationGui::new(),
        }
    }
}

impl eframe::App for GnssSdrRsGui {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("GNSS-SDR-RS");
            ui.separator();

            egui::Panel::top("acquisition_panel").show(ui, |ui| {
                self.acquisition_gui.show(ui);
            });
            // egui::TopBottomPanel::bottom("tracking_panel").show_inside(ui, |_ui| {
            //     self.tracking_gui.show(ui);
            // });
            // egui::TopBottomPanel::bottom("navigation_panel").show_inside(ui, |_ui| {
            //     self.navigation_gui.show(ui);
            // });

        });
    }
}