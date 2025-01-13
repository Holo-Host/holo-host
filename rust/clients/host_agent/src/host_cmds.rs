use crate::agent_cli::HostCommands;
use hpos_hal::inventory::HoloInventory;

pub fn host_command(command: &HostCommands) -> Result<(), std::io::Error> {
    // TODO: Fill these in under a separate set of commits to keep PRs simple.
    match command {
        HostCommands::ModelInfo => {
            let i = HoloInventory::from_host();
            //println!("{}", serde_json::to_string(&i).unwrap());
            match i.platform {
                Some(p) => {
                    println!("{}", p)
                }
                None => {
                    println!("No platform information retrieved.")
                }
            }
        }
    }
    Ok(())
}
