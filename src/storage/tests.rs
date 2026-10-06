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
// 2. Path & Collision Tests
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
// 3. Storage CRUD & Iteration Tests
// ==========================================

#[test]
fn test_storage_create_and_read_note() {
    let data_dir = tempdir().unwrap();
    let config_dir = tempdir().unwrap();

    let storage = Storage {
        _config_dir: config_dir.path().to_path_buf(),
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
        _config_dir: config_dir.path().to_path_buf(),
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
        _config_dir: config_dir.path().to_path_buf(),
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
        _config_dir: config_dir.path().to_path_buf(),
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
        _config_dir: config_dir.path().to_path_buf(),
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
