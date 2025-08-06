use clap::{Subcommand};
use squashfs::{
    create_archive,
    unpack_archive
};

#[derive(Subcommand)]
pub enum StaticContentCommand {
    Pack {
        #[arg(short, long)]
        source: String,

        #[arg(short, long)]
        fallback: Option<String>,

        #[arg(short, long)]
        destination: Option<String>
    },
    Unpack {
        #[arg(short, long)]
        package: String,

        #[arg(short, long)]
        destination: Option<String>
    }
}

pub fn handle_static_content_command(command: StaticContentCommand) {
    match command {
        StaticContentCommand::Pack { source, fallback, destination } => {
            let destination = match destination {
                Some(d) => d,
                None => "./package.pack".to_string()
            };
            create_archive(source, destination).expect("failed to create package")
        },
        StaticContentCommand::Unpack { package, destination } => {
            let destination = match destination {
                Some(d) => d,
                None => "./unpack".to_string()
            };
            unpack_archive(package, destination).expect("failed to unpack package")
        }
    }
}