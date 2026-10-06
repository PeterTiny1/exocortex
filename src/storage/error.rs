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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};
    use std::path::PathBuf;

    #[test]
    fn test_path_io_context_ok() {
        let res: std::io::Result<i32> = Ok(42);
        let path = Path::new("/tmp/test.txt");

        assert_eq!(res.with_path(path).unwrap(), 42);
    }

    #[test]
    fn test_path_io_context_err() {
        let io_err = IoError::new(ErrorKind::NotFound, "file missing");
        let res: std::io::Result<()> = Err(io_err);
        let path = Path::new("/tmp/missing.txt");

        let err = res.with_path(path).unwrap_err();

        match err {
            Error::Io {
                path: err_path,
                source,
            } => {
                assert_eq!(err_path, PathBuf::from("/tmp/missing.txt"));
                assert_eq!(source.kind(), ErrorKind::NotFound);
            }
            _ => panic!("expected Error::Io variant"),
        }
    }

    #[test]
    fn test_error_display_formatting() {
        // Test HomeNotFound
        assert_eq!(
            Error::HomeNotFound.to_string(),
            "could not determine user home directory"
        );

        // Test NoteNotFound
        let note_err = Error::NoteNotFound {
            slug: "my-first-note".into(),
        };
        assert_eq!(note_err.to_string(), "note not found: my-first-note");

        // Test Io error formatting
        let io_err = Error::Io {
            path: PathBuf::from("/etc/config.toml"),
            source: IoError::new(ErrorKind::PermissionDenied, "access denied"),
        };
        assert_eq!(
            io_err.to_string(),
            "failed IO operation at /etc/config.toml: access denied"
        );
    }
}
