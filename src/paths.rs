use std::env;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AlpnestPaths {
    pub home: PathBuf,
    pub contents_dir: PathBuf,
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
}

impl AlpnestPaths {
    pub fn resolve() -> io::Result<Self> {
        let home = match env::var_os("ALPNEST_HOME") {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => default_alpnest_home()?,
        };

        let contents_dir = home.join("contents");
        let config_dir = match env::var_os("ALPNEST_CONFIG_DIR") {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => home.join("config"),
        };
        let config_file = config_dir.join("alpnest.toml");

        Ok(Self {
            home,
            contents_dir,
            config_dir,
            config_file,
        })
    }
}

fn default_alpnest_home() -> io::Result<PathBuf> {
    let user_home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;

    #[cfg(target_os = "macos")]
    {
        Ok(user_home
            .join("Library")
            .join("Application Support")
            .join("alpnest"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME") {
            Ok(PathBuf::from(xdg_data_home).join("alpnest"))
        } else {
            Ok(user_home.join(".local").join("share").join("alpnest"))
        }
    }
}
