extern crate holochain_agent;
extern crate holochain_cas_implementations;
extern crate holochain_core;
extern crate holochain_core_api;
extern crate holochain_core_types;
extern crate holochain_dna;

extern crate dirs;
extern crate jsonrpc_ws_server;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate ws;

mod ws_server;

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

use jsonrpc_ws_server::{
    ServerBuilder,
    jsonrpc_core::{
        IoHandler,
        Value,
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

use ws_server::{
    HcDex
};

const DATA_DIR: &str = ".holo-host";

fn get_context(agent: Agent) -> HcResult<Context> {
    let path = data_dir()?.join(agent.to_string());
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

fn install_dna(dna_str: &str) -> Result<Dna, serde_json::Error> {
    let dna = Dna::from_json_str(dna_str)?;
    Ok(dna)
}

fn create_holochain(dna: &Dna, context: Context) -> HolochainResult<Holochain> {
    Holochain::new(dna.clone(), Arc::new(context))
}

fn create_dir_ignore_existing(path: &PathBuf) -> io::Result<()> {
    fs::create_dir(&path).or_else(|err| match err.kind() {
        io::ErrorKind::AlreadyExists => Ok(()),
        _ => Err(err)
    })
}

fn data_dir() -> io::Result<PathBuf> {
    let dir: PathBuf = dirs::home_dir()
        .expect("No home directory!?")
        .join(DATA_DIR);
    create_dir_ignore_existing(&dir)?;
    Ok(dir)
}


fn main () -> io::Result<()> {
    let host = Agent::from("hoster".to_string());
    let context = get_context(host).unwrap();

    let agents = ["agent1"].iter().map(|a| Agent::from(a.to_string()));

    let dna = install_dna(
        include_str!("../sample/app1.dna.json")
    ).unwrap();

    let holochains: HcDex = agents.map(|agent| {
        let dna_hash = dna.to_entry().address().to_string();
        let agent_hash = agent.to_string();
        let context = get_context(agent).unwrap();
        let hc = create_holochain(&dna, context).unwrap();
        ((agent_hash, dna_hash), RefCell::new(hc))
    }).collect();

    let server = ws_server::start_ws_server("3000", &holochains);
    Ok(())

}
