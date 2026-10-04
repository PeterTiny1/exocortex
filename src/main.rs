use crate::storage::Storage;

mod storage;

fn main() -> Result<(), storage::Error> {
    let storage = Storage::init()?;
    storage.create_note("Hello World", "First note")?;
    Ok(())
}
