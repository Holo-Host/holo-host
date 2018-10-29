use std::cell::RefCell;
use std::collections::HashMap;
use std::slice::IterMut;
// use std::sync::mpsc::{channel, Sender, Receiver};

use serde_json::{self, Value};
use ws::{
    self,
    Result as WsResult,
    Message,
};

use holochain_core::{
    context::Context,
    logger::SimpleLogger,
    persister::SimplePersister,
};
use holochain_core_api::{
    Holochain,
};
use holochain_core_types::{
    error::HolochainError,
};
use holochain_dna::{
    zome::capabilities::Membrane,
    Dna,
};

type RpcSegments = (String, String, String, String);

#[derive(Serialize, Deserialize)]
struct JsonRpc {
    jsonrpc: String,
    method: String,
    params: Value,
    id: u32,
}

fn parse_jsonrpc(s: &str) -> Result<JsonRpc, String> {
    let msg: JsonRpc = serde_json::from_str(s).map_err(|e| e.to_string())?;
    if msg.jsonrpc != "2.0" {
        Err("JSONRPC version must be 2.0".to_string())
    } else {
        Ok(msg)
    }
}

fn invoke_call(holochains: &HcDex, rpc_method: &str, params: Value) -> Result<Value, HolochainError> {
    let matches: Vec<&str> = rpc_method.split('/').collect();
    let result = if let [agent, dna, zome, cap, func] = matches.as_slice() {
        let hc_cell = holochains.get(&(agent.to_string(), dna.to_string())).expect("No instance for agent/dna pair");
        hc_cell.borrow_mut().call(zome, cap, func, &params.to_string()).map(Value::from).map_err(|e| e.to_string())
    } else {
        Err("bad rpc method".to_string())
    };
    result.map_err(HolochainError::ErrorGeneric)
}

pub type HcDex = HashMap<(String, String), RefCell<Holochain>>;

pub fn start_ws_server(
    port: &str,
    holochains: &HcDex,
) -> WsResult<()> {
    ws::listen(format!("localhost:{}", port), |out| {
        move |msg| {
            match msg {
                Message::Text(s) => {
                    match parse_jsonrpc(s.as_str()) {
                        Ok(rpc) => {
                            let response = match invoke_call(&holochains, rpc.method.as_str(), rpc.params) {
                                Ok(payload) => payload,
                                Err(err) => mk_err(&err.to_string())
                            };
                            out.send(Message::Text(response.to_string()))
                        },
                        Err(err) => {
                            out.send(Message::Text(mk_err(&err).to_string()))
                        }
                    }
                },
                Message::Binary(b) => Ok(())
            }
        }
    })
}

fn mk_err(msg: &str) -> Value {
    json!({
        "error": Value::from(msg)
    })
}

fn rpc_method_name(dna_name:&String, zome_name:&String, cap_name:&String, func_name:&String) -> String {
    format!("{}/{}/{}/{}", dna_name, zome_name, cap_name, func_name)
}
