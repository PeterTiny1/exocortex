use clap::{Parser, Subcommand};
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

use crate::storage::{Note, Storage};

mod storage;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Exocortex: augmented memory and productivity engine"
)]
pub struct Cli {
    /// Custom path to storage directory
    #[arg(short, long, value_name = "DIR", global = true)]
    pub storage_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new note
    #[command(alias = "new")]
    Create {
        /// Title of the note
        #[arg(short, long)]
        title: Option<String>,

        /// Optional raw content or body (reads stdin if omitted when piped)
        content: Option<String>,
    },
    /// List all notes in storage
    #[command(alias = "ls")]
    List,
    /// Read and display a note's raw content by slug
    #[command(alias = "cat")]
    Read {
        /// Slug of the note to display
        slug: String,
    },
    /// Delete a note by slug
    #[command(aliases = ["rm", "del"])]
    Delete {
        /// Slug of the note to remove
        slug: String,
    },
    /// Search notes by title or content substring
    Search {
        /// Search query
        query: String,
    },
    /// Rename a note (updates title frontmatter and filename slug)
    #[command(alias = "mv")]
    Rename {
        /// Current slug of the note
        slug: String,
        /// New title for the note
        new_title: String,
    },
    /// Update the content of a note
    #[command(alias = "edit")]
    Update {
        /// Slug of the note to update
        slug: String,
        /// Content to replace current content with
        new_content: Option<String>,
    },
}

fn read_content_or_stdin(content: Option<String>) -> io::Result<String> {
    match content {
        Some(c) => Ok(c),
        None if !io::stdin().is_terminal() => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            Ok(buffer)
        }
        None => Ok(String::new()),
    }
}

fn main() -> anyhow::Result<()> {
    let parsed = Cli::parse();
    let storage = Storage::init(parsed.storage_dir)?;

    match parsed.command {
        Commands::Create { title, content } => {
            let title = title.unwrap_or_else(|| "Untitled Note".to_string());
            let content = read_content_or_stdin(content)?;

            let note = Note::new(&title, &content);
            let stored = storage.create_note(note)?;

            println!("Created note at: {}", stored.path().display());
        }
        Commands::List => {
            let notes = storage.list_notes()?;
            if notes.is_empty() {
                println!("No notes found.");
            } else {
                for stored in notes {
                    println!("{} ({})", stored.note.title, stored.slug());
                }
            }
        }
        Commands::Read { slug } => {
            let stored = storage.read_note(&slug)?;
            println!("# {}\n\n{}", stored.note.title, stored.note.content);
        }
        Commands::Delete { slug } => {
            storage.delete_note(&slug)?;
            println!("Deleted {slug}.md");
        }
        Commands::Search { query } => {
            let results = storage.search_notes(&query)?;
            if results.is_empty() {
                println!("No notes matched '{query}'");
            } else {
                for res in results {
                    let match_type = if res.title_match { "title" } else { "content" };
                    println!(
                        "{} ({}) [{match_type} match]",
                        res.note.note.title,
                        res.note.slug()
                    );
                }
            }
        }
        Commands::Rename { slug, new_title } => {
            let updated = storage.rename_note(&slug, &new_title)?;
            println!(
                "Renamed '{}' -> '{}' ({}.md)",
                slug,
                updated.note.title,
                updated.slug()
            );
        }
        Commands::Update { slug, new_content } => {
            let content = read_content_or_stdin(new_content)?;
            storage.update_note_content(&slug, &content)?;
            println!("Changed content of '{}.md'", slug);
        }
    }

    Ok(())
}
