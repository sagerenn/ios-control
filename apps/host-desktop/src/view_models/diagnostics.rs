#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsViewModel {
    pub host_error: Option<String>,
    pub control_summary: String,
    pub grounding_summary: String,
}
