use egui::Ui;

#[derive(Debug, Clone, PartialEq)]
pub struct GroundingDiagnosticsViewModel {
    pub pointer_uncertainty: f32,
    pub focus_confidence: f32,
    pub last_failure: Option<String>,
}

impl GroundingDiagnosticsViewModel {
    pub fn summary(&self) -> String {
        format!(
            "pointer uncertainty {:.1}, focus {:.2}, last failure {}",
            self.pointer_uncertainty,
            self.focus_confidence,
            self.last_failure
                .clone()
                .unwrap_or_else(|| "none".to_string())
        )
    }
}

pub fn render(ui: &mut Ui, message: &str) {
    ui.heading("Diagnostics");
    ui.label(message);
}

pub fn render_control_diagnostics(ui: &mut Ui, message: &str) {
    ui.heading("Control Diagnostics");
    ui.label(message);
}
