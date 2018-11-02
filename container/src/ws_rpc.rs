use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::slice::IterMut;
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
use util::*;

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
    pub fn with_holochains(holochain_map: HolochainMap) -> Self {
        println!("Loaded holochains:");
        for (agent, dna) in holochain_map.keys() {
            println!("+  {} ({})", dna, agent);
        }
        Self { holochain_map }
    }

    fn call_rpc(&self, rpc_method: &str, params: Value) -> Result<Value, HolochainError> {
        let matches: Vec<&str> = rpc_method.split('/').collect();
        let result = if let [agent, dna, zome, cap, func] = matches.as_slice() {
            let key = (agent.to_string(), dna.to_string());
            self.holochain_map
                .get(&key)
                .ok_or(format!("No instance for agent/dna pair: {:?}", key))
                .and_then(|hc_cell| {
                    hc_cell
                        .borrow_mut()
                        .call(zome, cap, func, &params.to_string())
                        .map(Value::from)
                        .map_err(|e| e.to_string())
                })
        } else {
            Err(format!("bad rpc method: {}", rpc_method))
        };
        result.map_err(HolochainError::ErrorGeneric)
    }

    pub fn start_holochains(&self) -> HolochainResult<&Self> {
        self.holochain_map
            .values()
            .map(|cell| cell.borrow_mut().start())
            .into_iter()
            .collect::<Result<Vec<()>, HolochainInstanceError>>()
            .map(|_| self)
    }

    pub fn stop_holochains(&self) -> HolochainResult<&Self> {
        self.holochain_map
            .values()
            .map(|cell| cell.borrow_mut().stop())
            .into_iter()
            .collect::<Result<Vec<()>, HolochainInstanceError>>()
            .map(|_| self)
    }

    pub fn serve(&self, port: &str) -> WsResult<()> {
        ws::listen(format!("localhost:{}", port), |out| {
            move |msg| match msg {
                Message::Text(s) => match parse_jsonrpc(s.as_str()) {
                    Ok(rpc) => {
                        let response = match self.call_rpc(rpc.method.as_str(), rpc.params) {
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

    /// Build a HolochainMap from the cross product of two vectors
    fn holochain_map_from_product(agents: Vec<Agent>, dnas: Vec<Dna>) -> HolochainMap {
        let agents = agents.into_iter().map(Rc::new);
        let dnas: Vec<Rc<Dna>> = dnas.into_iter().map(Rc::new).collect();
        let mut pairs = Vec::new();
        for agent in agents {
            for dna in dnas.iter() {
                pairs.push((agent.clone(), dna.clone()));
            }
        }
        make_holochain_map(pairs)
    }

    fn mock_data() -> (Vec<Agent>, Vec<Dna>, HolochainMap) {
        let agents: Vec<Agent> = ["a1", "a2"]
            .iter()
            .map(|a| Agent::from(a.to_string()))
            .collect();
        let dnas = vec![Dna::from_json_str(include_str!("../sample/app1.dna.json")).unwrap()];
        let holochain_map = holochain_map_from_product(agents.clone(), dnas.clone());
        (agents, dnas, holochain_map)
    }

    // #[test]
    // fn can_start_server() {
    //     let (agents, dnas, holochain_map) = mock_data();
    //     HcWebsocketRpcServer::with_holochains(holochain_map)
    //         .start_holochains()
    //         .unwrap()
    //         .serve("4321")
    //         .unwrap();
    // }

    #[test]
    fn can_call_holochains() {
        let (agents, dnas, holochain_map) = mock_data();
        let server = HcWebsocketRpcServer::with_holochains(holochain_map);
        server.start_holochains().unwrap();

        let method = format!(
            "{}/{}/blog/main/create_post",
            agents[0].to_string(),
            get_dna_hash(&dnas[0]),
        );
        let params = json!({
            "content": "i see you",
            "in_reply_to": "the moon",
        });
        let result = server.call_rpc(&method, params).unwrap();
        println!("TODO - check once this is not an error: {:?}", result);
    }
}
