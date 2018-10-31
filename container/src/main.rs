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

mod ws_rpc;

use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    io,
    path::PathBuf,
    result::Result,
    sync::{
        Arc,
        Mutex,
    }
};

use holochain_agent::Agent;
use holochain_cas_implementations::{
    cas::file::FilesystemStorage, eav::file::EavFileStorage, path::create_path_if_not_exists,
};
use holochain_core::{
    context::Context,
    logger::SimpleLogger,
    persister::SimplePersister,
};
use holochain_core_api::{
    Holochain,
    error::{
        HolochainResult,
        HolochainInstanceError,
    }
};
use holochain_core_types::{
    cas::content::AddressableContent,
    entry::ToEntry,
    error::{
        HolochainError,
        HcResult,
    }
};
use holochain_dna::Dna;

use ws_rpc::{
    HolochainMap,
    HcWebsocketRpcServer,
};

const CONTEXT_DIR: &str = ".holo-host/context";
const DNA_DIR: &str = ".holo-host/dnas";
const HOST_IDENTITY: &str = "hoster";

fn get_context(agent: Agent) -> HcResult<Context> {
    let path = get_agent_path(&agent)?;
    create_dir_ignore_existing(&path)?;

    let stringify = |path: PathBuf| path.to_str().unwrap().to_owned();
    let cas_path = stringify(path.join("cas"));
    let eav_path = stringify(path.join("eav"));
    let agent_path = stringify(path.join("state"));
    create_path_if_not_exists(cas_path.as_str())?;
    create_path_if_not_exists(eav_path.as_str())?;
    Context::new(
        agent,
        Arc::new(Mutex::new(SimpleLogger {})),
        Arc::new(Mutex::new(SimplePersister::new(agent_path))),
        FilesystemStorage::new(&cas_path)?,
        EavFileStorage::new(eav_path.into())?,
    )
}

fn get_agent_path(agent: &Agent) -> io::Result<PathBuf> {
    get_context_dir().map(|dir| dir.join(agent.to_string()))
}

/// TODO: add entry to hosting app
fn install_dna(dna_str: &str) -> Result<Dna, serde_json::Error> {
    let dna = Dna::from_json_str(dna_str)?;
    Ok(dna)
}

fn create_holochain(dna: &Dna, context: Context) -> HolochainResult<Holochain> {
    Holochain::new(dna.clone(), Arc::new(context))
}

fn create_dir_ignore_existing(path: &PathBuf) -> io::Result<()> {
    fs::create_dir_all(&path).or_else(|err| match err.kind() {
        io::ErrorKind::AlreadyExists => Ok(()),
        _ => Err(err)
    })
}

fn get_context_dir() -> io::Result<PathBuf> {
    let dir: PathBuf = dirs::home_dir()
        .expect("No home directory!?")
        .join(CONTEXT_DIR);
    create_dir_ignore_existing(&dir)?;
    Ok(dir)
}

fn main () -> io::Result<()> {
    let host_agent = Agent::from(HOST_IDENTITY.to_string());

    let dna = install_dna(
        include_str!("../sample/app1.dna.json")
    ).unwrap();
    let dna_hash = dna.to_entry().address().to_string();
    println!("Loaded DNA: {}", dna_hash);

    let agents = ["agent1"].iter().map(|a| Agent::from(a.to_string()));
    let mut holochain_map: HolochainMap = agents.map(|agent| {
        let agent_hash = agent.to_string();
        let context = get_context(agent).unwrap();
        let hc = create_holochain(&dna, context).unwrap();
        println!("Made instance for agent: {}", agent_hash);
        ((agent_hash, dna_hash.clone()), RefCell::new(hc))
    }).collect();

    let host_context = get_context(host_agent.clone()).unwrap();
    let host_hc = create_holochain(&dna, host_context).unwrap();
    holochain_map.insert((host_agent.to_string(), dna_hash), RefCell::new(host_hc));
    println!("Made instance for host: {}", host_agent.to_string());

    HcWebsocketRpcServer::with_holochains(holochain_map)
        .start_holochains().expect("Could not start holochains!")
        .serve("3000").expect("Could not start websocket server");
    Ok(())

}
