pub(crate) enum Overlay {
    Confirm { command: String },
    Result { output: String, scroll: u16 },
}

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) selected: usize,
    pub(crate) detail: bool,
    pub(crate) status_message: String,
    pub(crate) overlay: Option<Overlay>,
}
