#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    MainExplorer,
    ContentEditor,
    BuildPanel,
    CookSection,
    ConfigureMail,
    Settings,
}

impl Default for AppView {
    fn default() -> Self {
        Self::MainExplorer
    }
}

impl AppView {
    pub fn title(self) -> &'static str {
        match self {
            Self::MainExplorer => "Main Explorer",
            Self::ContentEditor => "Add / Edit Content",
            Self::BuildPanel => "Build Panel",
            Self::CookSection => "Cook Section",
            Self::ConfigureMail => "Configure Mail",
            Self::Settings => "Settings",
        }
    }
}
