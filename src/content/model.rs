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

#[derive(Debug, Clone)]
pub struct Panel {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub prompt_path: Option<PathBuf>,
    pub sections: Vec<Section>,
    pub order: i32,
    pub hidden: bool,
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
