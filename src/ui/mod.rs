use eframe::egui::{self, Color32, RichText};
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug)]
pub enum UiMessage {
    SetStatus(String),
    SetProgress(f32),
    SetDownloadProgress { progress: f32, speed_kbps: f64 },
    ShowError(String),
    Close,
}

pub struct RunnerApp {
    status: String,
    progress: f32,
    error: Option<String>,
    download_speed: Option<f64>,
    receiver: Receiver<UiMessage>,
    sender: Sender<UiMessage>,
}

impl RunnerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Set window size
        cc.egui_ctx.set_pixels_per_point(1.0);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        
        // Set initial window size
        cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(400.0, 100.0)));

        let (sender, receiver) = channel();
        
        Self {
            status: String::from("Initializing..."),
            progress: 0.0,
            error: None,
            download_speed: None,
            receiver,
            sender,
        }
    }

    pub fn sender(&self) -> Sender<UiMessage> {
        self.sender.clone()
    }
}

impl eframe::App for RunnerApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Process any pending messages
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                UiMessage::SetStatus(status) => self.status = status,
                UiMessage::SetProgress(progress) => self.progress = progress,
                UiMessage::SetDownloadProgress { progress, speed_kbps } => {
                    self.progress = progress;
                    self.download_speed = Some(speed_kbps);
                },
                UiMessage::ShowError(error) => self.error = Some(error),
                UiMessage::Close => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                },
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                if let Some(error) = &self.error {
                    ui.label(RichText::new(error).color(Color32::RED));
                    if ui.button("Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                } else {
                    ui.label(&self.status);
                    ui.add_space(10.0);
                    
                    ui.add(egui::ProgressBar::new(self.progress)
                        .show_percentage()
                        .animate(true));
                        
                    if let Some(speed) = self.download_speed {
                        ui.label(format!("Download speed: {:.2} KB/s", speed));
                    }
                }
            });
        });

        // Request a repaint
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_messages() {
        let (tx, rx) = channel();
        
        tx.send(UiMessage::SetProgress(0.5)).unwrap();
        tx.send(UiMessage::SetStatus("Testing".to_string())).unwrap();
        
        assert!(matches!(rx.recv().unwrap(), UiMessage::SetProgress(0.5)));
        assert!(matches!(rx.recv().unwrap(), UiMessage::SetStatus(s) if s == "Testing"));
    }
} 