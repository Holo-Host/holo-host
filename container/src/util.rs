
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    io,
    rc::Rc,
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


const CONTEXT_DIR: &str = ".holo-host/context";
const DNA_DIR: &str = ".holo-host/dnas";
pub const HOST_IDENTITY: &str = "hoster";

/// Unique identifier for a Holochain: (AGENT_KEY, DNA_HASH)
pub type InstanceKey = (String, String);

/// Map from keys to internally mutable Holochains
pub type HolochainMap = HashMap<InstanceKey, RefCell<Holochain>>;


pub fn get_agent_path(agent: &Agent) -> io::Result<PathBuf> {
    get_context_dir().map(|dir| dir.join(agent.to_string()))
}

pub fn create_holochain(dna: &Dna, context: Context) -> HolochainResult<Holochain> {
    Holochain::new(dna.clone(), Arc::new(context))
}

pub fn create_dir_ignore_existing(path: &PathBuf) -> io::Result<()> {
    fs::create_dir_all(&path).or_else(|err| match err.kind() {
        io::ErrorKind::AlreadyExists => Ok(()),
        _ => Err(err)
    })
}

pub fn get_context_dir() -> io::Result<PathBuf> {
    let dir: PathBuf = dirs::home_dir()
        .expect("No home directory!?")
        .join(CONTEXT_DIR);
    create_dir_ignore_existing(&dir)?;
    Ok(dir)
}

pub fn get_context(agent: &Agent) -> HcResult<Context> {
    let path = get_agent_path(agent)?;
    create_dir_ignore_existing(&path)?;

    let stringify = |path: PathBuf| path.to_str().unwrap().to_owned();
    let cas_path = stringify(path.join("cas"));
    let eav_path = stringify(path.join("eav"));
    let agent_path = stringify(path.join("state"));
    create_path_if_not_exists(cas_path.as_str())?;
    create_path_if_not_exists(eav_path.as_str())?;
    Context::new(
        agent.clone(),
        Arc::new(Mutex::new(SimpleLogger {})),
        Arc::new(Mutex::new(SimplePersister::new(agent_path))),
        FilesystemStorage::new(&cas_path)?,
        EavFileStorage::new(eav_path.into())?,
    )
}

pub fn get_dna_hash(dna: &Dna) -> String { dna.to_entry().address().to_string() }

pub fn make_holochain_map(pairs: Vec<(Rc<Agent>, Rc<Dna>)>) -> HolochainMap {
    pairs.iter().map(|(agent, dna)| {
        let agent_hash = agent.to_string();
        let dna_hash = dna.to_entry().address().to_string();
        let context = get_context(agent).unwrap();
        let hc = create_holochain(&dna, context).unwrap();
        println!("Made instance for agent: {}", agent_hash);
        ((agent_hash, dna_hash.clone()), RefCell::new(hc))
    }).collect()
}
