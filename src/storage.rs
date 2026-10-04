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

#[derive(Debug, Clone)]
pub struct Note {
    pub title: String,
    pub content: String,
}

impl Note {
    pub fn new(title: &str, content: &str) -> Self {
        Self {
            title: title.to_owned(),
            content: content.to_owned(),
        }
    }

    fn parse(raw_str: &str, fallback_title: &str) -> Self {
        let normalized = raw_str.replace("\r\n", "\n");
        let trimmed = normalized.trim();

        let mut title = fallback_title.to_string();

        let content = if let Some(rest) = trimmed.strip_prefix("---\n")
            && let Some((frontmatter, note_contents)) = rest.split_once("\n---\n")
        {
            for line in frontmatter.lines() {
                if let Some((field_name, field_content)) = line.split_once(':')
                    && field_name.trim() == "title"
                {
                    title = field_content.trim().trim_matches(['"', '\'']).to_string();
                }
            }
            note_contents.trim().to_string()
        } else {
            trimmed.to_string()
        };

        Self { title, content }
    }

    pub fn to_file_content(&self) -> String {
        format!("---\ntitle: {}\n---\n\n{}\n", self.title, self.content)
    }
}

fn generate_unique_path(base_path: &Path, slug: &str) -> PathBuf {
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

#[derive(Debug, Clone)]
pub struct StoredNote {
    pub note: Note,
    path: PathBuf,
}

impl StoredNote {
    fn new(note: Note, base_path: &Path) -> Self {
        let slug = slugify(&note.title);

        let path = generate_unique_path(base_path, &slug);

        Self { path, note }
    }

    fn write(&self) -> Result<(), Error> {
        fs::write(&self.path, self.note.to_file_content()).map_err(|e| Error::Io {
            path: self.path.clone(),
            source: e,
        })?;
        Ok(())
    }

    fn from_file(path: &Path) -> Result<Self, Error> {
        let file_contents = fs::read_to_string(path).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let fallback_title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");
        Ok(Self {
            path: path.to_path_buf(),
            note: Note::parse(&file_contents, fallback_title),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn slugify(title: &str) -> String {
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

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

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

        create_dir_all(&config_dir)?;
        create_dir_all(&data_dir)?;

        Ok(Self {
            config_dir,
            data_dir,
        })
    }

    pub fn create_note(&self, note: Note) -> Result<StoredNote, Error> {
        let stored = StoredNote::new(note, &self.data_dir);
        stored.write()?;
        Ok(stored)
    }

    pub fn list_notes(&self) -> Result<Vec<StoredNote>, Error> {
        let notes = read_dir(&self.data_dir)?
            .filter_map(|entry| {
                let path = entry.ok()?.path();

                if path.is_file() && path.extension() == Some(OsStr::new("md")) {
                    StoredNote::from_file(&path).ok()
                } else {
                    None
                }
            })
            .collect();

        Ok(notes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("  Rust 2024 -- Edition  "), "rust-2024-edition");
        assert_eq!(slugify("!!!"), "untitled");
    }

    #[test]
    fn test_note_parse_with_frontmatter() {
        let raw = "---\ntitle: Custom Title\n---\n\nNote content here.";
        let note = Note::parse(raw, "fallback");
        assert_eq!(note.title, "Custom Title");
        assert_eq!(note.content, "Note content here.");
    }

    #[test]
    fn test_collision_handling() {
        let temp_dir = std::env::temp_dir().join("exocortex_test_collisions");
        let _ = fs::remove_dir_all(&temp_dir); // Ensure clean state from any prior failed run
        fs::create_dir_all(&temp_dir).unwrap();

        let path1 = generate_unique_path(&temp_dir, "test-note");
        fs::write(&path1, "content").unwrap();

        let path2 = generate_unique_path(&temp_dir, "test-note");
        assert_eq!(path2.file_name().unwrap(), "test-note-1.md");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_note_parse_fallback() {
        let raw = "Just raw markdown without frontmatter.";
        let note = Note::parse(raw, "Fallback Title");
        assert_eq!(note.title, "Fallback Title");
        assert_eq!(note.content, "Just raw markdown without frontmatter.");
    }

    #[test]
    fn test_multiple_collisions() {
        let temp_dir = std::env::temp_dir().join("exocortex_test_multi_collisions");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let path0 = generate_unique_path(&temp_dir, "note");
        fs::write(&path0, "c0").unwrap();

        let path1 = generate_unique_path(&temp_dir, "note");
        fs::write(&path1, "c1").unwrap();

        let path2 = generate_unique_path(&temp_dir, "note");
        assert_eq!(path2.file_name().unwrap(), "note-2.md");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
