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
        let slug = slug_from_path(path, "untitled");
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // ==========================================
    // 1. Note Parsing Tests
    // ==========================================

    #[test]
    fn test_parse_valid_frontmatter() {
        let raw = "---\ntitle: \"My Daily Note\"\n---\n\nThis is the content.";
        let note = Note::parse(raw, "fallback");

        assert_eq!(note.title, "My Daily Note");
        assert_eq!(note.content, "This is the content.");
    }

    #[test]
    fn test_parse_single_quoted_title() {
        let raw = "---\ntitle: 'Single Quoted'\n---\nNote content here.";
        let note = Note::parse(raw, "fallback");

        assert_eq!(note.title, "Single Quoted");
        assert_eq!(note.content, "Note content here.");
    }

    #[test]
    fn test_parse_title_with_colons() {
        // Frontmatter fields often contain colons in values (e.g., subtitles/timestamps)
        let raw = "---\ntitle: \"Rust: Advanced Patterns & Tips\"\n---\nContent";
        let note = Note::parse(raw, "fallback");

        assert_eq!(note.title, "Rust: Advanced Patterns & Tips");
    }

    #[test]
    fn test_parse_crlf_line_endings() {
        let raw = "---\r\ntitle: Windows Style\r\n---\r\n\r\nContent with CRLF";
        let note = Note::parse(raw, "fallback");

        assert_eq!(note.title, "Windows Style");
        assert_eq!(note.content, "Content with CRLF");
    }

    #[test]
    fn test_parse_missing_frontmatter_uses_fallback() {
        let raw = "Just plain text without frontmatter delimiters.";
        let note = Note::parse(raw, "fallback-slug");

        assert_eq!(note.title, "fallback-slug");
        assert_eq!(
            note.content,
            "Just plain text without frontmatter delimiters."
        );
    }

    #[test]
    fn test_parse_malformed_frontmatter_does_not_panic() {
        // Missing closing delimiter
        let raw = "---\ntitle: Unclosed Frontmatter\nContent without end block";
        let note = Note::parse(raw, "fallback");

        assert_eq!(note.title, "fallback");
        assert_eq!(note.content, raw.trim());
    }

    #[test]
    fn test_note_roundtrip_serialization() {
        let original = Note::new("Roundtrip Test", "Line 1\nLine 2");
        let serialized = original.to_file_content();
        let parsed = Note::parse(&serialized, "fallback");

        assert_eq!(parsed.title, original.title);
        assert_eq!(parsed.content, original.content);
    }

    // ==========================================
    // 2. StoredNote & Persistence Tests
    // ==========================================

    #[test]
    fn test_stored_note_with_path() {
        let note = Note::new("Inline Note", "Some text");
        let path = PathBuf::from("/tmp/my-note.md");
        let stored = StoredNote::with_path(note, &path);

        assert_eq!(stored.path(), path);
        assert!(!stored.slug().is_empty());
    }

    #[test]
    fn test_stored_note_file_io() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_note.md");

        let note = Note::new("File System Note", "Testing disk write and read.");
        let stored = StoredNote::with_path(note, &file_path);

        // Write to disk
        stored.write().unwrap();
        assert!(file_path.exists());

        // Read back from disk
        let loaded = StoredNote::from_file(&file_path).unwrap();
        assert_eq!(loaded.note.title, "File System Note");
        assert_eq!(loaded.note.content, "Testing disk write and read.");
        assert_eq!(loaded.path(), file_path);
    }

    #[test]
    fn test_stored_note_from_nonexistent_file_fails() {
        let nonexistent = Path::new("/nonexistent_path_12345/note.md");
        let result = StoredNote::from_file(nonexistent);

        assert!(result.is_err());
        if let Err(Error::Io { path, .. }) = result {
            assert_eq!(path, nonexistent);
        } else {
            panic!("Expected Error::Io with attached path context");
        }
    }
}
