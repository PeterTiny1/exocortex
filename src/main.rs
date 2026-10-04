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
            println!(
                "{}",
                storage
                    .list_notes()?
                    .into_iter()
                    .map(|note| note.note.title)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    }
    Ok(())
}
