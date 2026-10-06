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
