use crate::storage::{PathError, Storage};

mod storage;

fn main() -> Result<(), PathError> {
    let storage = Storage::init()?;
    Ok(())
}
