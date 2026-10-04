use clap::{Parser, Subcommand};
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
    #[arg(short, long, value_name = "DIR")]
    pub storage_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new note
    Create {
        /// Title of the note
        #[arg(short, long)]
        title: Option<String>,

        /// Optional raw content or body
        content: Option<String>,
    },
    /// List all notes in storage
    List,
    /// Read and display a note's raw content by slug
    Read {
        /// Slug of the note to display
        slug: String,
    },
    /// Delete a note by slug
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
    Rename {
        /// Current slug of the note
        slug: String,
        /// New title for the note
        new_title: String,
    },
}

fn main() -> Result<(), storage::Error> {
    let parsed = Cli::parse();
    let storage = Storage::init(parsed.storage_dir)?;
    match parsed.command {
        Commands::Create { title, content } => {
            // Resolve defaults at the CLI boundary
            let title = title.unwrap_or_else(|| "Untitled Note".to_string());
            let content = content.unwrap_or_default();

            let note = Note::new(&title, &content);
            let stored = storage.create_note(note)?;

            println!("Created note at: {}", stored.path().display());
        }
        Commands::List => {
            for stored in storage.list_notes()? {
                println!("{} ({})", stored.note.title, stored.slug());
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
    }
    Ok(())
}
