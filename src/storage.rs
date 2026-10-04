use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("could not determine user home directory")]
    HomeNotFound,
    #[error("failed to create directory at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub struct Storage {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}
impl Storage {
    pub fn init() -> Result<Self, PathError> {
        let config_dir = locate_dir(dirs::config_dir())?;
        let data_dir = match read_custom_data_dir(&config_dir) {
            Some(custom_path) => custom_path,
            None => locate_dir(dirs::data_local_dir())?,
        };
        create_dir_all(&config_dir)?;
        create_dir_all(&data_dir)?;
        Ok(Self {
            config_dir,
            data_dir,
        })
    }
}

fn read_custom_data_dir(_config_dir: &Path) -> Option<PathBuf> {
    // Stub: We will parse config.toml here once we add serde/toml
    None
}

fn create_dir_all(dir: &Path) -> Result<(), PathError> {
    std::fs::create_dir_all(dir).map_err(|e| PathError::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

fn locate_dir(path: Option<PathBuf>) -> Result<PathBuf, PathError> {
    Ok(path.ok_or(PathError::HomeNotFound)?.join(CRATE_NAME))
}
