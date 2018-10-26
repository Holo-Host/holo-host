extern crate holochain_agent;
extern crate holochain_cas_implementations;
extern crate holochain_core;
extern crate holochain_core_api;
extern crate holochain_core_types;
extern crate holochain_dna;

extern crate dirs;

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

fn get_context(path: PathBuf, agent: Agent) -> HcResult<Context> {
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
    let hc = Holochain::new(dna.clone(), Arc::new(context))?.into();
    Ok((dna, hc))
}

fn create_dir_ignore_existing(path: &PathBuf) -> io::Result<()> {
    fs::create_dir(&path).or_else(|err| match err.kind() {
        io::ErrorKind::AlreadyExists => Ok(()),
        _ => Err(err)
    })
}

fn main () -> io::Result<()> {
    let agent = Agent::from("hoster".to_string());
    let data_dir: PathBuf = dirs::home_dir()
        .expect("No home directory!?")
        .join(DATA_DIR);
    create_dir_ignore_existing(&data_dir)?;

    let host_data_dir = data_dir.join("hostland");
    create_dir_ignore_existing(&host_data_dir)?;

    let context = get_context(host_data_dir, agent).unwrap();
    let (dna, hc) = install_dna(
        include_str!("../sample/app1.dna.json"),
        context
    ).unwrap();
    Ok(())

}
