
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use crate::data::acquisition_data::AcquisitionData;

pub struct AcquisitionGui {
    data: std::sync::mpsc::Receiver<AcquisitionData>,
}

impl AcquisitionGui {
    pub fn new(rx_gui: std::sync::mpsc::Receiver<AcquisitionData>) -> Self {
        Self {
            data: rx_gui,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("GPS Acquisition");

        self.show_data(ui);
    }

    fn show_data(&self, ui: &mut egui::Ui) {
        while let Ok(d) = self.data.try_recv() {
            if d.fft_power.is_empty() {
                ui.label("No FFT data");
                return;
            }

            let points: PlotPoints = d.fft_power
                .iter()
                .enumerate()
                .map(|(i, power)| {
                    [i as f64, *power as f64]
                })
                .collect();

            let line = Line::new("FFT", points);

            Plot::new("acquisition_fft")
                .view_aspect(2.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(line);
                });
        }
    }
}