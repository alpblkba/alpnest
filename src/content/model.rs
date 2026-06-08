use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
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
