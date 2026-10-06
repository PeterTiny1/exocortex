use tempfile::NamedTempFile;

use crate::storage::{Error, error::PathIoContext};
use std::{
    fs::{self, ReadDir},
    path::{Path, PathBuf},
};

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

pub fn locate_dir(path: Option<PathBuf>) -> Result<PathBuf, Error> {
    let base = path.ok_or(Error::HomeNotFound)?;
    Ok(base.join(CRATE_NAME))
}

// Standard I/O wrappers with context attached
pub fn read_dir(dir: &Path) -> Result<ReadDir, Error> {
    fs::read_dir(dir).with_path(dir)
}

pub fn create_dir_all(dir: &Path) -> Result<(), Error> {
    fs::create_dir_all(dir).with_path(dir)
}

pub fn read_to_string(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).with_path(path)
}

pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    fs::write(path, contents).with_path(path)
}

pub fn remove_file(path: &Path) -> Result<(), Error> {
    std::fs::remove_file(path).with_path(path)
}

// tempfile wrappers
pub fn temp_file_write_all(file: &mut NamedTempFile, contents: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    file.write_all(contents).with_path(file.path())
}

pub fn new_temp_file(path: &Path) -> Result<NamedTempFile, Error> {
    NamedTempFile::new_in(path).with_path(path)
}

pub fn persist_temp_file(temp_file: NamedTempFile, target: &Path) -> Result<(), Error> {
    temp_file.persist(target).map_err(|err| Error::Io {
        path: target.to_path_buf(),
        source: err.error,
    })?;
    Ok(())
}

pub fn atomic_write(path: &Path, dir: &Path, data: &[u8]) -> Result<(), Error> {
    let mut temp_file = new_temp_file(dir)?;
    temp_file_write_all(&mut temp_file, data)?;
    persist_temp_file(temp_file, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_locate_dir() {
        let path = PathBuf::from("/home/user");
        let result = locate_dir(Some(path)).unwrap();
        assert_eq!(result, PathBuf::from("/home/user").join(CRATE_NAME));

        let err = locate_dir(None).unwrap_err();
        assert!(matches!(err, Error::HomeNotFound));
    }

    #[test]
    fn test_file_io_operations() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        // Write & Read string
        write(&file_path, "hello world").unwrap();
        let contents = read_to_string(&file_path).unwrap();
        assert_eq!(contents, "hello world");

        // Remove file
        remove_file(&file_path).unwrap();
        assert!(!file_path.exists());

        // Verify Error context path mapping on failure
        let err = read_to_string(&file_path).unwrap_err();
        if let Error::Io { path, source } = err {
            assert_eq!(path, file_path);
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
        } else {
            panic!("Expected Error::Io");
        }
    }

    #[test]
    fn test_directory_operations() {
        let dir = tempdir().unwrap();
        let nested_dir = dir.path().join("a").join("b");

        // Create directory tree
        create_dir_all(&nested_dir).unwrap();
        assert!(nested_dir.is_dir());

        // Read directory entries
        let entries: Vec<_> = read_dir(dir.path()).unwrap().map(|r| r.unwrap()).collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name(), "a");
    }

    #[test]
    fn test_atomic_write() {
        let dir = tempdir().unwrap();
        let target_file = dir.path().join("atomic.txt");
        let payload = b"atomic payload";

        // Successful atomic write
        atomic_write(&target_file, dir.path(), payload).unwrap();
        assert_eq!(fs::read(&target_file).unwrap(), payload);

        // Failure case: target directory does not exist for temp file creation
        let invalid_dir = dir.path().join("non_existent");
        let err = atomic_write(&target_file, &invalid_dir, payload).unwrap_err();

        if let Error::Io { path, .. } = err {
            assert_eq!(path, invalid_dir);
        } else {
            panic!("Expected Error::Io containing invalid directory path");
        }
    }
}
