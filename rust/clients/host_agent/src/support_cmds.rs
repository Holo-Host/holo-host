use crate::agent_cli::SupportCommands;

pub fn support_command(command: &SupportCommands) -> Result<(), std::io::Error> {
    // TODO: Fill these in under a separate set of commits to keep PRs simple.
    match command {
        SupportCommands::NetTest => {
            println!("Network Test not yet supported")
        }
    }
    Ok(())
}
