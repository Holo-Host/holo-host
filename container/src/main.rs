extern crate holochain_agent;
extern crate holochain_cas_implementations;
extern crate holochain_core;
extern crate holochain_core_api;
extern crate holochain_core_types;
extern crate holochain_dna;

extern crate dirs;
extern crate jsonrpc_ws_server;
extern crate serde_json;

mod ws;

use std::{
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
use holochain_core_types::error::{
    HolochainError,
    HcResult,
};
use holochain_dna::Dna;

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

fn install_dna(dna_str: &str, context: Context) -> HolochainResult<(Dna, Holochain)> {
    let dna = Dna::from_json_str(include_str!("../sample/app1.dna.json"))
        .map_err(|err| HolochainError::ErrorGeneric(err.to_string()))
        .map_err(HolochainInstanceError::from)?;
    let hc = create_holochain(dna, context)?.into();
    Ok((dna, hc))
}

fn create_holochain(dna: Dna, context: Context) -> HolochainResult<Holochain> {
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

    let agents = ["agent1", "agent2", "agent3"].iter().map(|a| Agent::from(a.to_string()));
    
    let (dna, hc) = install_dna(
        include_str!("../sample/app1.dna.json"),
        context
    ).unwrap();

    let holochains: Vec<(Holochain, Dna)> = agents.map(|agent| {
        let context = get_context(agent).unwrap();
        let hc = create_holochain(dna, context).unwrap();
        (hc, dna)
    }).collect();

    let ws_server = ws::start_ws_server("3000", holochains, vec![dna].into_iter())
        .expect("WebSocket server could not start");
    ws_server.wait().unwrap();
    Ok(())

}
