extern crate holochain_agent;
extern crate holochain_cas_implementations;
extern crate holochain_core;
extern crate holochain_core_api;
extern crate holochain_core_types;
extern crate holochain_dna;

use std::path::PathBuf;
use std::sync::{
    Arc,
    Mutex,
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
use holochain_core_api::Holochain;
use holochain_core_types::error::{
    HolochainError,
    HcResult,
};
use holochain_dna::Dna;


fn get_context(path: String, agent: Agent) -> HcResult<Context> {
    let cas_path = format!("{}/cas", path);
    let eav_path = format!("{}/eav", path);
    let agent_path = format!("{}/state", path);
    create_path_if_not_exists(&cas_path)?;
    create_path_if_not_exists(&eav_path)?;
    Context::new(
        agent,
        Arc::new(Mutex::new(SimpleLogger {})),
        Arc::new(Mutex::new(SimplePersister::new(agent_path))),
        FilesystemStorage::new(&cas_path)?,
        EavFileStorage::new(eav_path)?,
    )
}


fn main () {
     let agent = Agent::from("hoster".to_string());
     let context = get_context("testenv/hostland".to_string(), agent).unwrap();
     let dna = Dna::from_json_str(include_str!("../sample/app1.dna.json")).unwrap();
     let hc = Holochain::new(dna, Arc::new(context));
}
