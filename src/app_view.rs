#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    MainExplorer,
    AddContent,
    EditContent,
    BuildPanel,
    CookSection,
    ConfigureMail,
    Calendar,
}

impl Default for AppView {
    fn default() -> Self {
        Self::MainExplorer
    }
}
