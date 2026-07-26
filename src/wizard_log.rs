//! Shared notification log for the build/cook wizards.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Note,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Note => "[note]",
            Self::Info => "[info]",
            Self::Warning => "[warning]",
            Self::Error => "[ERROR]",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
        }
    }
}

/// Wizards keep a bounded scrollback; older lines fall off the top.
pub fn trim(logs: &mut Vec<LogEntry>, max: usize) {
    if logs.len() > max {
        let drain_count = logs.len() - max;
        logs.drain(0..drain_count);
    }
}
