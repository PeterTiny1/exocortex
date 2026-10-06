use std::path::{Path, PathBuf};

use crate::storage::{Error, generate_unique_path, io, slug_from_path, slugify};

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

    pub fn parse(raw_str: &str, fallback_title: &str) -> Self {
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

#[derive(Debug, Clone)]
pub struct StoredNote {
    pub note: Note,
    path: PathBuf,
    slug: String,
}

impl StoredNote {
    pub fn new(note: Note, base_path: &Path) -> Self {
        let base_slug = slugify(&note.title);
        let path = generate_unique_path(base_path, &base_slug);
        let slug = slug_from_path(&path, &base_slug);

        Self { note, path, slug }
    }

    pub fn from_file(path: &Path) -> Result<Self, Error> {
        let file_contents = io::read_to_string(path)?;
        let slug = slug_from_path(path, "untitled");
        Ok(Self {
            path: path.to_path_buf(),
            note: Note::parse(&file_contents, &slug),
            slug,
        })
    }

    pub fn with_path(note: Note, path: &Path) -> Self {
        let slug = slug_from_path(&path, "untitled");
        Self {
            path: path.to_path_buf(),
            slug,
            note,
        }
    }

    pub fn write(&self) -> Result<(), Error> {
        io::write(&self.path, self.note.to_file_content())?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub note: StoredNote,
    pub title_match: bool,
}
