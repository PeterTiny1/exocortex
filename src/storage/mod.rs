mod error;
mod io;
mod model;
#[cfg(test)]
mod tests;

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub use crate::storage::model::{Note, SearchResult, StoredNote};
pub use error::Error;
use error::PathIoContext;
use io::locate_dir;

pub struct Storage {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Storage {
    pub fn init(cli_data_dir: Option<PathBuf>) -> Result<Self, Error> {
        let config_dir = locate_dir(dirs::config_dir())?;

        let data_dir = match cli_data_dir {
            Some(path) => path,
            None => match read_custom_data_dir(&config_dir) {
                Some(custom_path) => custom_path,
                None => locate_dir(dirs::data_local_dir())?,
            },
        };

        io::create_dir_all(&config_dir)?;
        io::create_dir_all(&data_dir)?;

        Ok(Self {
            config_dir,
            data_dir,
        })
    }

    /// Resolves a note slug to its existing file path, returning an error if missing.
    pub fn get_note_path(&self, slug: &str) -> Result<PathBuf, Error> {
        let safe_slug = Path::new(slug)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| Error::NoteNotFound {
                slug: slug.to_string(),
            })?;

        let path = self.data_dir.join(format!("{safe_slug}.md"));
        if !path.is_file() {
            return Err(Error::NoteNotFound {
                slug: slug.to_string(),
            });
        }
        Ok(path)
    }

    pub fn create_note(&self, note: Note) -> Result<StoredNote, Error> {
        let stored = StoredNote::new(note, &self.data_dir);
        stored.write()?;
        Ok(stored)
    }

    /// Streams notes one-by-one lazily without allocating a temporary Vec.
    pub fn iter_notes(&self) -> Result<impl Iterator<Item = Result<StoredNote, Error>>, Error> {
        let entries = io::read_dir(&self.data_dir)?;

        let iter = entries.filter_map(|entry| {
            let entry = match entry.with_path(&self.data_dir) {
                Ok(e) => e,
                Err(err) => return Some(Err(err)),
            };

            let path = entry.path();
            if path.is_file() && path.extension() == Some(OsStr::new("md")) {
                Some(StoredNote::from_file(&path))
            } else {
                None
            }
        });

        Ok(iter)
    }

    pub fn list_notes(&self) -> Result<Vec<StoredNote>, Error> {
        self.iter_notes()?.collect()
    }

    pub fn search_notes(&self, query: &str) -> Result<Vec<SearchResult>, Error> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for note in self.iter_notes()? {
            let note = note?;
            let title_match = note.note.title.to_lowercase().contains(&query_lower);
            let content_match = note.note.content.to_lowercase().contains(&query_lower);

            if title_match || content_match {
                results.push(SearchResult { note, title_match });
            }
        }

        Ok(results)
    }

    pub fn read_note(&self, slug: &str) -> Result<StoredNote, Error> {
        StoredNote::from_file(&self.get_note_path(slug)?)
    }

    pub fn delete_note(&self, slug: &str) -> Result<(), Error> {
        let path = self.get_note_path(slug)?;
        io::remove_file(&path)
    }

    pub fn rename_note(&self, old_slug: &str, new_title: &str) -> Result<StoredNote, Error> {
        let old_stored = self.read_note(old_slug)?;

        let new_slug_base = slugify(new_title);
        let new_path = generate_unique_path(&self.data_dir, &new_slug_base);

        let new_stored =
            StoredNote::with_path(Note::new(new_title, &old_stored.note.content), &new_path);

        new_stored.write()?;

        if old_stored.path() != new_path
            && let Err(e) = io::remove_file(&old_stored.path())
        {
            let _ = io::remove_file(&new_path);
            return Err(e);
        }

        Ok(new_stored)
    }

    pub fn update_note_content(&self, slug: &str, new_content: &str) -> Result<(), Error> {
        let path = self.get_note_path(slug)?;
        let mut stored = StoredNote::from_file(&path)?;
        stored.note.content = new_content.to_string();

        io::atomic_write(
            &path,
            &self.data_dir,
            stored.note.to_file_content().as_bytes(),
        )
    }
}

// Utility Functions
pub(crate) fn slugify(title: &str) -> String {
    let slug = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

pub(crate) fn slug_from_path(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(fallback)
        .to_string()
}

pub(crate) fn generate_unique_path(base_path: &Path, slug: &str) -> PathBuf {
    let mut candidate = base_path.join(format!("{slug}.md"));
    if !candidate.exists() {
        return candidate;
    }

    let mut counter = 1;
    loop {
        candidate = base_path.join(format!("{slug}-{counter}.md"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn read_custom_data_dir(_config_dir: &Path) -> Option<PathBuf> {
    None
}
