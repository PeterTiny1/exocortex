mod error;
use std::{
    ffi::OsStr,
    fs::{self, ReadDir},
    path::{Path, PathBuf},
};

pub use error::Error;
use error::PathIoContext;
use tempfile::NamedTempFile;

// ==========================================
// 1. Constants & Error Definitions
// ==========================================

const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

// ==========================================
// 2. Domain Models (Note, StoredNote, SearchResult)
// ==========================================

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

    fn from_file(path: &Path) -> Result<Self, Error> {
        let file_contents = read_to_string(path)?;
        let slug = slug_from_path(path, "untitled");
        Ok(Self {
            path: path.to_path_buf(),
            note: Note::parse(&file_contents, &slug),
            slug,
        })
    }

    fn write(&self) -> Result<(), Error> {
        write(&self.path, self.note.to_file_content())?;
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

// ==========================================
// 3. Primary Business Logic (Storage)
// ==========================================

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

    /// Resolves a note slug to its existing file path, returning an error if missing.
    pub fn get_note_path(&self, slug: &str) -> Result<PathBuf, Error> {
        // Prevent path traversal attacks
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
        let entries = read_dir(&self.data_dir)?;

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
            let note = note?; // Explicitly propagate read/parse errors
            let title_match = note.note.title.to_lowercase().contains(&query_lower);
            let content_match = note.note.content.to_lowercase().contains(&query_lower);

            if title_match || content_match {
                results.push(SearchResult { note, title_match });
            }
        }

        Ok(results)
    }

    /// Read a stored note by its unique slug.
    pub fn read_note(&self, slug: &str) -> Result<StoredNote, Error> {
        StoredNote::from_file(&self.get_note_path(slug)?)
    }

    /// Delete a stored note from disk by its slug.
    pub fn delete_note(&self, slug: &str) -> Result<(), Error> {
        let path = self.get_note_path(slug)?;
        remove_file(&path)
    }

    /// Renames a note by updating its title, writing the new file first,
    /// and removing the old file only after the new file is safely on disk.
    pub fn rename_note(&self, old_slug: &str, new_title: &str) -> Result<StoredNote, Error> {
        let old_stored = self.read_note(old_slug)?;

        let new_slug_base = slugify(new_title);
        let new_path = generate_unique_path(&self.data_dir, &new_slug_base);
        let slug = slug_from_path(&new_path, &new_slug_base);

        // 1. Create updated StoredNote pointing to the new path
        let new_stored = StoredNote {
            path: new_path.clone(),
            slug,
            note: Note {
                title: new_title.to_string(),
                content: old_stored.note.content.clone(),
            },
        };

        // 2. Write the new note file first
        new_stored.write()?;

        // 3. Only delete old file if path changed and write succeeded
        if old_stored.path != new_path
            && let Err(e) = remove_file(&old_stored.path)
        {
            // If cleanup fails, attempt to rollback new file to prevent duplicate state
            let _ = remove_file(&new_path);
            return Err(e);
        }

        Ok(new_stored)
    }

    /// Updates the content of a note
    pub fn update_note_content(&self, slug: &str, new_content: &str) -> Result<(), Error> {
        let path = self.get_note_path(slug)?;
        let mut stored = StoredNote::from_file(&path)?;
        stored.note.content = new_content.to_string();

        // Unique temporary file in the same directory for atomic rename
        let mut temp_file = new_temp_file(&self.data_dir)?;

        temp_file_write_all(&mut temp_file, stored.note.to_file_content().as_bytes())?;

        temp_file.persist(&path).map_err(|err| Error::Io {
            path,
            source: err.error,
        })?;

        Ok(())
    }
}

// ==========================================
// 4. Utility Functions & I/O Helpers
// ==========================================

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

pub fn slug_from_path(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(fallback)
        .to_string()
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

fn locate_dir(path: Option<PathBuf>) -> Result<PathBuf, Error> {
    Ok(path.ok_or(Error::HomeNotFound)?.join(CRATE_NAME))
}

fn read_custom_data_dir(_config_dir: &Path) -> Option<PathBuf> {
    // Stub: We will parse config.toml here once we add serde/toml
    None
}

// Standard I/O wrappers with context attached
fn read_dir(dir: &Path) -> Result<ReadDir, Error> {
    fs::read_dir(dir).with_path(dir)
}

fn create_dir_all(dir: &Path) -> Result<(), Error> {
    fs::create_dir_all(dir).with_path(dir)
}

fn read_to_string(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).with_path(path)
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    fs::write(path, contents).with_path(path)
}

fn temp_file_write_all(file: &mut NamedTempFile, contents: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    file.write_all(contents).with_path(file.path())
}

fn remove_file(path: &Path) -> Result<(), Error> {
    std::fs::remove_file(path).with_path(path)
}

fn new_temp_file(path: &Path) -> Result<NamedTempFile, Error> {
    NamedTempFile::new_in(path).with_path(path)
}

// ==========================================
// 5. Tests
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // ==========================================
    // 1. Slugification Tests
    // ==========================================

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World!"), "hello-world");
        assert_eq!(slugify("  Rust 2024 -- Edition  "), "rust-2024-edition");
        assert_eq!(slugify("!!!"), "untitled");
        assert_eq!(slugify("---special---chars---"), "special-chars");
        assert_eq!(slugify("   "), "untitled");
    }

    #[test]
    fn test_slugify_unicode_and_emojis() {
        // Non-ASCII characters are filtered out by is_ascii_alphanumeric()
        assert_eq!(slugify("🦀 Rust & C++ 🔥"), "rust-c");
        assert_eq!(slugify("Café & Résumé"), "caf-r-sum");
    }

    // ==========================================
    // 2. Note Parsing & Serialization Tests
    // ==========================================

    #[test]
    fn test_note_parse_with_frontmatter() {
        let raw = "---\ntitle: Custom Title\n---\n\nNote content here.";
        let note = Note::parse(raw, "fallback");
        assert_eq!(note.title, "Custom Title");
        assert_eq!(note.content, "Note content here.");
    }

    #[test]
    fn test_note_parse_frontmatter_quoted_titles() {
        let raw_double = "---\ntitle: \"Double Quoted Title\"\n---\n\nContent";
        let note_double = Note::parse(raw_double, "fallback");
        assert_eq!(note_double.title, "Double Quoted Title");

        let raw_single = "---\ntitle: 'Single Quoted Title'\n---\n\nContent";
        let note_single = Note::parse(raw_single, "fallback");
        assert_eq!(note_single.title, "Single Quoted Title");
    }

    #[test]
    fn test_note_parse_fallback() {
        let raw = "Just raw markdown without frontmatter.";
        let note = Note::parse(raw, "Fallback Title");
        assert_eq!(note.title, "Fallback Title");
        assert_eq!(note.content, "Just raw markdown without frontmatter.");
    }

    #[test]
    fn test_note_parse_unclosed_frontmatter() {
        let raw = "---\ntitle: Incomplete Frontmatter\nNo closing fence";
        let note = Note::parse(raw, "fallback");
        assert_eq!(note.title, "fallback");
        assert_eq!(note.content, raw);
    }

    #[test]
    fn test_note_parse_crlf_normalization() {
        let raw = "---\r\ntitle: Windows Line Endings\r\n---\r\n\r\nContent with CRLF.\r\n";
        let note = Note::parse(raw, "fallback");
        assert_eq!(note.title, "Windows Line Endings");
        assert_eq!(note.content, "Content with CRLF.");
    }

    #[test]
    fn test_note_to_file_content() {
        let note = Note::new("My Title", "My Content");
        let content = note.to_file_content();
        assert_eq!(content, "---\ntitle: My Title\n---\n\nMy Content\n");
    }

    // ==========================================
    // 3. Path & Collision Tests
    // ==========================================

    #[test]
    fn test_collision_handling() {
        let dir = tempdir().unwrap();

        let path1 = generate_unique_path(dir.path(), "test-note");
        assert_eq!(path1.file_name().unwrap(), "test-note.md");
        fs::write(&path1, "content").unwrap();

        let path2 = generate_unique_path(dir.path(), "test-note");
        assert_eq!(path2.file_name().unwrap(), "test-note-1.md");
    }

    #[test]
    fn test_multiple_collisions() {
        let dir = tempdir().unwrap();

        let path0 = generate_unique_path(dir.path(), "note");
        fs::write(&path0, "c0").unwrap();

        let path1 = generate_unique_path(dir.path(), "note");
        fs::write(&path1, "c1").unwrap();

        let path2 = generate_unique_path(dir.path(), "note");
        assert_eq!(path2.file_name().unwrap(), "note-2.md");
    }

    #[test]
    fn test_slug_from_path() {
        let path = Path::new("/some/dir/my-cool-note.md");
        assert_eq!(slug_from_path(path, "fallback"), "my-cool-note");

        let invalid_path = Path::new("/");
        assert_eq!(slug_from_path(invalid_path, "fallback"), "fallback");
    }

    // ==========================================
    // 4. StoredNote Operations Tests
    // ==========================================

    #[test]
    fn test_stored_note_write_and_read() {
        let dir = tempdir().unwrap();
        let note = Note::new("Test Note", "Sample body text");

        let stored = StoredNote::new(note, dir.path());
        assert_eq!(stored.slug(), "test-note");
        stored.write().unwrap();

        assert!(stored.path().exists());

        let read_back = StoredNote::from_file(stored.path()).unwrap();
        assert_eq!(read_back.note.title, "Test Note");
        assert_eq!(read_back.note.content, "Sample body text");
        assert_eq!(read_back.slug(), "test-note");
    }

    // ==========================================
    // 5. Storage CRUD & Iteration Tests
    // ==========================================

    #[test]
    fn test_storage_create_and_read_note() {
        let data_dir = tempdir().unwrap();
        let config_dir = tempdir().unwrap();

        let storage = Storage {
            config_dir: config_dir.path().to_path_buf(),
            data_dir: data_dir.path().to_path_buf(),
        };

        let note = Note::new("Storage Test", "Testing storage workflows.");
        let stored = storage.create_note(note).unwrap();

        let fetched = storage.read_note("storage-test").unwrap();
        assert_eq!(fetched.note.title, "Storage Test");
        assert_eq!(fetched.note.content, "Testing storage workflows.");
        assert_eq!(fetched.path(), stored.path());
    }

    #[test]
    fn test_storage_list_notes() {
        let data_dir = tempdir().unwrap();
        let config_dir = tempdir().unwrap();

        let storage = Storage {
            config_dir: config_dir.path().to_path_buf(),
            data_dir: data_dir.path().to_path_buf(),
        };

        storage
            .create_note(Note::new("Note 1", "Content 1"))
            .unwrap();
        storage
            .create_note(Note::new("Note 2", "Content 2"))
            .unwrap();

        // Non-markdown file and subfolder should be ignored by list_notes
        fs::write(storage.data_dir.join("ignore.txt"), "ignore me").unwrap();
        fs::create_dir(storage.data_dir.join("subfolder")).unwrap();

        let notes = storage.list_notes().unwrap();
        assert_eq!(notes.len(), 2);

        let titles: Vec<String> = notes.into_iter().map(|n| n.note.title).collect();
        assert!(titles.contains(&"Note 1".to_string()));
        assert!(titles.contains(&"Note 2".to_string()));
    }

    #[test]
    fn test_storage_search_notes() {
        let data_dir = tempdir().unwrap();
        let config_dir = tempdir().unwrap();

        let storage = Storage {
            config_dir: config_dir.path().to_path_buf(),
            data_dir: data_dir.path().to_path_buf(),
        };

        storage
            .create_note(Note::new(
                "Rust Programming",
                "Systems language focused on safety.",
            ))
            .unwrap();
        storage
            .create_note(Note::new(
                "Cooking Recipes",
                "How to make rust-style sourdough bread.",
            ))
            .unwrap();
        storage
            .create_note(Note::new("Python Tips", "Dynamic scripting."))
            .unwrap();

        // Title-only match
        let results = storage.search_notes("Programming").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title_match);
        assert_eq!(results[0].note.note.title, "Rust Programming");

        // Content-only match
        let results = storage.search_notes("sourdough").unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].title_match);
        assert_eq!(results[0].note.note.title, "Cooking Recipes");

        // Case-insensitive title and content matches
        let results = storage.search_notes("rust").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_storage_delete_note() {
        let data_dir = tempdir().unwrap();
        let config_dir = tempdir().unwrap();

        let storage = Storage {
            config_dir: config_dir.path().to_path_buf(),
            data_dir: data_dir.path().to_path_buf(),
        };

        storage
            .create_note(Note::new("To Delete", "Goodbye"))
            .unwrap();
        assert!(storage.get_note_path("to-delete").is_ok());

        storage.delete_note("to-delete").unwrap();
        assert!(matches!(
            storage.read_note("to-delete"),
            Err(Error::NoteNotFound { .. })
        ));
    }

    #[test]
    fn test_storage_note_not_found() {
        let data_dir = tempdir().unwrap();
        let config_dir = tempdir().unwrap();

        let storage = Storage {
            config_dir: config_dir.path().to_path_buf(),
            data_dir: data_dir.path().to_path_buf(),
        };

        let result = storage.get_note_path("non-existent-slug");
        assert!(matches!(
            result,
            Err(Error::NoteNotFound { slug }) if slug == "non-existent-slug"
        ));
    }

    #[test]
    fn test_storage_init_with_cli_dir() {
        let cli_dir = tempdir().unwrap().path().join("custom_data");

        let storage = Storage::init(Some(cli_dir.clone())).unwrap();
        assert_eq!(storage.data_dir, cli_dir);
        assert!(storage.data_dir.exists());
    }

    // ==========================================
    // 6. Error & Edge Case Tests
    // ==========================================

    #[test]
    fn test_read_dir_non_existent() {
        let missing_path = Path::new("/non/existent/path/for/exocortex/tests");
        let storage = Storage {
            config_dir: PathBuf::new(),
            data_dir: missing_path.to_path_buf(),
        };

        let result = storage.list_notes();
        assert!(matches!(result, Err(Error::Io { path, .. }) if path == missing_path));
    }
}
