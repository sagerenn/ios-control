use egui::Ui;
use ios_control_contracts::capture::VideoFrameDescriptor;

pub fn render(ui: &mut Ui, frame: Option<&VideoFrameDescriptor>) {
    ui.heading("Session View");
    if let Some(frame) = frame {
        ui.label(format!(
            "{}x{} frame {}",
            frame.width, frame.height, frame.frame_index
        ));
    } else {
        ui.label("No active frame source");
    }
}
