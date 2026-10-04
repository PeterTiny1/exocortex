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
    let slug = if slug.is_empty() { "untitled" } else { slug };
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

        Self {
            path,
            note,
        }
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
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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

    pub fn create_note(&self, title: &str, content: &str) -> Result<StoredNote, Error> {
        let note = Note::new(title, content);
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
