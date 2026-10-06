use tempfile::NamedTempFile;

use crate::storage::{Error, error::PathIoContext};
use std::{fs::{self, ReadDir}, path::{Path, PathBuf}};

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
