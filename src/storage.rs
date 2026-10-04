use std::{
    ffi::OsStr,
    fs::{self, ReadDir},
    path::{Path, PathBuf},
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("could not determine user home directory")]
    HomeNotFound,
    #[error("failed IO operation at {path}: {source}")]
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
    pub fn init() -> Result<Self, Error> {
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

    pub fn create_note(&self, title: &str, content: &str) -> Result<PathBuf, Error> {
        let filename = format!("{}.md", title.to_lowercase().replace(' ', "-"));
        let note_path = self.data_dir.join(filename);

        let file_contents = format!("---\ntitle: {title}\n---\n\n{content}\n");

        fs::write(&note_path, file_contents).map_err(|e| Error::Io {
            path: note_path.clone(),
            source: e,
        })?;
        Ok(note_path)
    }

    pub fn list_notes(&self) -> Result<Vec<PathBuf>, Error> {
        Ok(read_dir(&self.data_dir)?
            .filter_map(|f| {
                f.ok()
                    .map(|file| file.path())
                    .filter(|path| path.extension() == Some(OsStr::new("md")) && path.is_file())
            })
            .collect())
    }
}

fn read_custom_data_dir(_config_dir: &Path) -> Option<PathBuf> {
    // Stub: We will parse config.toml here once we add serde/toml
    None
}

fn read_dir(dir: &Path) -> Result<ReadDir, Error> {
    let read = fs::read_dir(dir).map_err(|e| Error::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    Ok(read)
}

fn create_dir_all(dir: &Path) -> Result<(), Error> {
    fs::create_dir_all(dir).map_err(|e| Error::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

fn locate_dir(path: Option<PathBuf>) -> Result<PathBuf, Error> {
    Ok(path.ok_or(Error::HomeNotFound)?.join(CRATE_NAME))
}
