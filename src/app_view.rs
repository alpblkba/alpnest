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

impl AppView {
    pub fn title(self) -> &'static str {
        match self {
            Self::MainExplorer => "Main Explorer",
            Self::AddContent => "Add Content",
            Self::EditContent => "Edit Content",
            Self::BuildPanel => "Build Panel",
            Self::CookSection => "Cook Section",
            Self::ConfigureMail => "Configure Mail",
            Self::Calendar => "Calendar",
        }
    }
}
