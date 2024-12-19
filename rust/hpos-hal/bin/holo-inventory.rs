/// This is a binary that simply gathers inventory from the current system, and writes it as JSON
/// to stdout. The primary use case is for Nix to be able to import the JSON and use it to control
/// Nix modules used to deploy/install/manage HPOS components regardless of the underlying hardware
/// and platform.
use hpos_hal::inventory::HoloInventory;

fn main() {
    env_logger::init();

    let i = HoloInventory::from_host();
    println!("{}", serde_json::to_string(&i).unwrap());
}
