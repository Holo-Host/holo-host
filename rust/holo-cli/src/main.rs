use clap::{Parser, Subcommand};
mod static_content;

#[derive(Parser)]
#[command(name = "static-content", about = "A static content server", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    StaticContent {
        #[command(subcommand)]
        subcommand: static_content::StaticContentCommand,
    },
}

fn main() {
    let args = Args::parse();
    match args.command {
        Commands::StaticContent { subcommand } => static_content::handle_static_content_command(subcommand)
    }
}