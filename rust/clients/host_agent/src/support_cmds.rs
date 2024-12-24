use crate::agent_cli::SupportCommands;

pub fn support_command(command: &SupportCommands) -> Result<(), std::io::Error> {
    // TODO: Fill these in under a separate set of commits to keep PRs simple.
    match command {
        SupportCommands::NetTest => {
            println!("Network Test not yet supported")
        }
        SupportCommands::SupportTunnel { enable } => {
            // This is independent of the implementation, which will be plumbed through once we
            // have an implementation for https://github.com/Holo-Host/holo-host-private/issues/14.
            match enable {
                true => { println!("Support Tunnel not yet implemented") }
                false => { println!("Support Tunnel already disabled") }
            }
        }
    }
    Ok(())
}
