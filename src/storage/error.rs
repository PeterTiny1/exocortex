use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not determine user home directory")]
    HomeNotFound,
    #[error("failed IO operation at {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("note not found: {slug}")]
    NoteNotFound { slug: String },
}

pub trait PathIoContext<T> {
    fn with_path(self, path: &Path) -> Result<T, Error>;
}

impl<T> PathIoContext<T> for std::io::Result<T> {
    fn with_path(self, path: &Path) -> Result<T, Error> {
        self.map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}
