use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Instant};
// use std::sync::mpsc::{channel, Sender, Receiver};

use serde_json::{self, Value};
use ws::{self, Message, Result as WsResult};

use holochain_agent::Agent;
use holochain_core::{context::Context, logger::SimpleLogger, persister::SimplePersister};
use holochain_core_api::{
    error::{HolochainInstanceError, HolochainResult},
    Holochain,
};
use holochain_core_types::error::HolochainError;
use holochain_dna::{zome::capabilities::Membrane, Dna};
use holo_dapp_hosts::{ServiceCycle, ServiceMetrics};

use util::{self, HolochainMap};

#[derive(Serialize, Deserialize)]
struct JsonRpc {
    jsonrpc: String,
    method: String,
    params: Value,
    id: u32,
}

pub struct HcWebsocketRpcServer {
    holochain_map: HolochainMap,
}

impl HcWebsocketRpcServer {
    /// Initialize struct with a map from Agent hash / DNA hash pairs
    /// to their corresponding `Holochain` instances
    pub fn new(holochain_map: HolochainMap) -> Self {
        println!("Loaded holochains:");
        for (agent, dna) in holochain_map.keys() {
            println!("+  {} ({})", dna, agent);
        }
        Self { holochain_map }
    }

    /// Start a websocket server which responds to JSONRPC frames, where `method` is:
    /// `[agent_key]/[dna_hash]/[zome_name]/[trait_name]/[function_name]`
    /// and `params` is a `serde_json::Value`
    pub fn serve(&self, port: &str) -> WsResult<()> {
        ws::listen(format!("localhost:{}", port), |out| {
            move |msg| match msg {
                Message::Text(s) => match parse_jsonrpc(s.as_str()) {
                    Ok(rpc) => {
                        let response = match self.dispatch_rpc(rpc.method.as_str(), rpc.params) {
                            Ok(payload) => payload,
                            Err(err) => mk_err(&err.to_string()),
                        };
                        out.send(Message::Text(response.to_string()))
                    }
                    Err(err) => out.send(Message::Text(mk_err(&err).to_string())),
                },
                Message::Binary(_b) => unimplemented!(),
            }
        })
    }

    /// Start all the Holochains
    pub fn start_holochains(&self) -> HolochainResult<&Self> {
        self.holochain_map
            .values()
            .map(|cell| cell.borrow_mut().start())
            .into_iter()
            .collect::<Result<Vec<()>, HolochainInstanceError>>()
            .map(|_| self)
    }

    /// Stop all the Holochains (this might be pretty useless)
    pub fn stop_holochains(&self) -> HolochainResult<&Self> {
        self.holochain_map
            .values()
            .map(|cell| cell.borrow_mut().stop())
            .into_iter()
            .collect::<Result<Vec<()>, HolochainInstanceError>>()
            .map(|_| self)
    }

    /// Dispatch to the correct Holochain and `call` it based on the JSONRPC method
    fn dispatch_rpc(&self, rpc_method: &str, params: Value) -> Result<Value, HolochainError> {
        let matches: Vec<&str> = rpc_method.split('/').collect();
        let result = if let [agent, dna, zome, cap, func] = matches.as_slice() {
            let key = (agent.to_string(), dna.to_string());
            self.holochain_map
                .get(&key)
                .ok_or(format!("No instance for agent/dna pair: {:?}", key))
                .and_then(|hc_cell| {
                    self.call_holochain(
                        &mut *hc_cell.borrow_mut(),
                        (zome, cap, func, params)
                    )
                })
        } else {
            Err(format!("bad rpc method: {}", rpc_method))
        };
        result.map_err(HolochainError::ErrorGeneric)
    }

    fn call_holochain(
        &self,
        hc: &mut Holochain,
        args: (&str, &str, &str, Value),
    ) -> Result<Value, String> {
        let (zome, cap, func, params) = args;
        let start_time = Instant::now();
        let result = hc
            .call(zome, cap, func, &params.to_string())
            .map(Value::from)
            .map_err(|e| e.to_string());
        if let Ok(response) = result {
            let duration = start_time.elapsed();
            let metrics = ServiceMetrics {
                cpu_time: duration.as_micros(),
                bytes_in: params.to_string().length(),
                bytes_out: response.to_string().length(),
            };
            println!("TODO: call actual hosting app instance");
            host_hc.call("hosts", "main", "log_service", json!({
                "agent_key": "TODO",
                "request": params,
                "response": response,
                "metrics": metrics
            }));
        }
        result
    }
}

fn parse_jsonrpc(s: &str) -> Result<JsonRpc, String> {
    let msg: JsonRpc = serde_json::from_str(s).map_err(|e| e.to_string())?;
    if msg.jsonrpc != "2.0" {
        Err("JSONRPC version must be 2.0".to_string())
    } else {
        Ok(msg)
    }
}

fn mk_err(msg: &str) -> Value {
    json!({ "error": Value::from(msg) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn mock_data(names: Vec<&str>) -> (Vec<Agent>, Vec<Dna>, HolochainMap) {
        let agents: Vec<Agent> = names.iter().map(|a| Agent::from(a.to_string())).collect();
        let dnas = vec![Dna::from_json_str(include_str!("../sample/app1.dna.json")).unwrap()];
        let holochain_map = util::holochain_map_from_product(agents.clone(), dnas.clone());
        (agents, dnas, holochain_map)
    }

    #[test]
    fn can_start_server() {
        let (agents, dnas, holochain_map) = mock_data(vec!["a1", "a2"]);
        let handle = thread::spawn(move || {
            HcWebsocketRpcServer::new(holochain_map)
                .start_holochains()
                .unwrap()
                .serve("4321")
                .unwrap();
        });
        handle.thread().unpark();
    }

    #[test]
    fn can_call_holochains() {
        let (agents, dnas, holochain_map) = mock_data(vec!["a3", "a4"]);
        let server = HcWebsocketRpcServer::new(holochain_map);
        server.start_holochains().unwrap();

        let method = format!(
            "{}/{}/blog/main/create_post",
            agents[0].to_string(),
            util::get_dna_hash(&dnas[0]),
        );
        let params = json!({
            "content": "i see you",
            "in_reply_to": "the moon",
        });
        let result = server.dispatch_rpc(&method, params).unwrap();
        println!("TODO - check once this is not an error: {:?}", result);
    }
}
