extern crate holochain_agent;
extern crate holochain_cas_implementations;
extern crate holochain_core;
extern crate holochain_core_api;
extern crate holochain_core_types;
extern crate holochain_dna;

extern crate dirs;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate ws;

pub mod util;
pub mod ws_rpc;

use std::{
    cell::RefCell,
    collections::HashMap,
    fs, io,
    path::PathBuf,
    result::Result,
    sync::{Arc, Mutex},
};

use holochain_agent::Agent;
use holochain_cas_implementations::{
    cas::file::FilesystemStorage, eav::file::EavFileStorage, path::create_path_if_not_exists,
};
use holochain_core::{context::Context, logger::SimpleLogger, persister::SimplePersister};
use holochain_core_api::{
    error::{HolochainInstanceError, HolochainResult},
    Holochain,
};
use holochain_core_types::{
    cas::content::AddressableContent,
    entry::ToEntry,
    error::{HcResult, HolochainError},
};
use holochain_dna::Dna;
use util::{create_holochain, get_context, HolochainMap};

use ws_rpc::HcWebsocketRpcServer;

/// TODO: add entry to hosting app
fn install_dna(dna_str: &str) -> Result<Dna, serde_json::Error> {
    let dna = Dna::from_json_str(dna_str)?;
    Ok(dna)
}

fn main() -> io::Result<()> {
    let host_agent = Agent::from(util::HOST_IDENTITY.to_string());

    let dna = install_dna(include_str!("../sample/app1.dna.json")).unwrap();
    let dna_hash = dna.to_entry().address().to_string();
    println!("Loaded DNA: {}", dna_hash);

    let agents = ["agent1"].iter().map(|a| Agent::from(a.to_string()));
    let mut holochain_map: HolochainMap = agents
        .map(|agent| {
            let agent_hash = agent.to_string();
            let context = get_context(&agent).unwrap();
            let hc = create_holochain(&dna, context).unwrap();
            println!("Made instance for agent: {}", agent_hash);
            ((agent_hash, dna_hash.clone()), RefCell::new(hc))
        })
        .collect();

    let host_context = get_context(&host_agent).unwrap();
    let host_hc = create_holochain(&dna, host_context).unwrap();
    holochain_map.insert((host_agent.to_string(), dna_hash), RefCell::new(host_hc));
    println!("Made instance for host: {}", host_agent.to_string());

    HcWebsocketRpcServer::new(holochain_map)
        .start_holochains()
        .expect("Could not start holochains!")
        .serve("3000")
        .expect("Could not start websocket server");
    Ok(())
}
