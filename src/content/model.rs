use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Minimal,
    Task,
    Project,
    Mail,
    Calendar,
    Unknown,
}

impl ContentType {
    pub fn from_manifest_value(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "minimal" => Self::Minimal,
            "task" => Self::Task,
            "project" => Self::Project,
            "mail" => Self::Mail,
            "calendar" => Self::Calendar,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Task => "task",
            Self::Project => "project",
            Self::Mail => "mail",
            Self::Calendar => "calendar",
            Self::Unknown => "unknown",
        }
    }

    /// Minimal content stops at overview/context. Mail and calendar render
    /// their own domain surfaces and also do not use the authored section
    /// grammar.
    pub fn has_sections(self) -> bool {
        !matches!(self, Self::Minimal | Self::Mail | Self::Calendar)
    }
}

#[derive(Debug, Clone)]
pub struct Content {
    pub id: String,
    pub title: String,
    pub content_type: ContentType,
    pub path: PathBuf,
    pub body_path: Option<PathBuf>,
    pub context_path: Option<PathBuf>,
    pub panels: Vec<Panel>,
    pub order: i32,
    pub hidden: bool,
}

impl Content {
    pub fn is_navigable(&self) -> bool {
        !self.hidden
    }
}

#[derive(Debug, Clone)]
pub struct Panel {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub prompt_path: Option<PathBuf>,
    pub sections: Vec<Section>,
    pub order: i32,
    pub hidden: bool,
    pub synthetic: bool,
}

impl Panel {
    pub fn is_navigable(&self) -> bool {
        !self.hidden
    }
}

#[derive(Debug, Clone)]
pub struct Section {
    pub id: String,
    pub title: String,
    pub body_path: PathBuf,
    pub context_path: Option<PathBuf>,
    pub order: i32,
    pub hidden: bool,
}

impl Section {
    pub fn is_navigable(&self) -> bool {
        !self.hidden
    }
}

#[cfg(test)]
mod type_tests {
    use super::ContentType;

    #[test]
    fn special_contents_do_not_have_sections() {
        assert!(!ContentType::Mail.has_sections());
        assert!(!ContentType::Calendar.has_sections());
    }

    #[test]
    fn authored_contents_have_sections() {
        assert!(ContentType::Task.has_sections());
        assert!(ContentType::Project.has_sections());
        assert!(ContentType::Unknown.has_sections());
        assert!(!ContentType::Minimal.has_sections());
    }

    #[test]
    fn manifest_values_round_trip_through_labels() {
        for kind in [
            ContentType::Minimal,
            ContentType::Task,
            ContentType::Project,
            ContentType::Mail,
            ContentType::Calendar,
        ] {
            assert_eq!(ContentType::from_manifest_value(kind.label()), kind);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContentManifest {
    pub schema_version: u32,
    pub id: Option<String>,
    pub title: Option<String>,
    pub content_type: Option<ContentType>,
    pub hidden: bool,
    pub order: i32,
}

impl Default for ContentManifest {
    fn default() -> Self {
        Self {
            schema_version: 1,
            id: None,
            title: None,
            content_type: None,
            hidden: false,
            order: 0,
        }
    }
}
