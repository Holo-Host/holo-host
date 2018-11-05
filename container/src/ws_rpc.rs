use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
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
use util::{self, HolochainMap};

#[derive(Serialize, Deserialize)]
struct JsonRpc {
    jsonrpc: String,
    method: String,
    params: Value,
    id: u32,
}

type BoxedCallback<P> = FnOnce(P) -> ();
pub type PreCallHook<'a> = &'a BoxedCallback<Value>;
pub type PostCallHook<'a> = &'a BoxedCallback<(Value, Value)>;

pub type CallResult = Result<Value, HolochainError>;

type Caller = Box<FnOnce(String, Value) -> CallResult>;
pub type CallClosure = Box<FnOnce() -> CallResult>;
pub type CallContext = fn(CallClosure) -> CallResult;

pub struct HcWebsocketRpcServer {
    holochain_map: HolochainMap,
    // pre_call_hook: Option<PreCallHook<'a>>,
    // post_call_hook: Option<PostCallHook<'a>>,
    // caller: Option<Caller>,
    call_context: Option<CallContext>,
}

impl HcWebsocketRpcServer {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            holochain_map: HashMap::new(),
            // pre_call_hook: None,
            // post_call_hook: None,
            // caller: None,
            call_context: None,
        })
    }

    /// Initialize struct with a map from Agent hash / DNA hash pairs
    /// to their corresponding `Holochain` instances
    pub fn with_holochains(mut self, holochain_map: HolochainMap) -> Self {
        println!("Loaded holochains:");
        for (agent, dna) in holochain_map.keys() {
            println!("+  {} ({})", dna, agent);
        }
        self.holochain_map = holochain_map;
        self
    }

    // pub fn with_pre_call_hook<'a>(mut self, pre_call_hook: PreCallHook<'a>) -> Self {
    //     self.pre_call_hook = Some(pre_call_hook);
    //     self
    // }

    // pub fn with_post_call_hook<'a>(mut self, post_call_hook: PostCallHook<'a>) -> Self {
    //     self.post_call_hook = Some(post_call_hook);
    //     self
    // }

    pub fn with_caller(mut self, func: CallContext) -> Self {
        // fn boxit<'a, F>(f: F) -> Box<F>
        // where
        //     F: FnOnce(String, Value) -> CallResult,
        // {
        //     Box::new(f)
        // }

        // let this = &self;
        // self.caller = Some(Box::new(|rpc_method: String, params: Value| {
        //     func(Box::new(move || this.call_rpc(rpc_method.as_str(), params)))
        // }));
        self.call_context = Some(func);
        self
        // self.caller = Box::new(|rpc_method, params| f(Box::new(move || self.call_rpc(rpc_method, params))));
    }

    fn do_call(&self, rpc_method: String, params: Value) -> CallResult {
        self.call_context.unwrap()(Box::new(move || self.call_rpc(rpc_method.as_str(), params)))
    }

    /// Start a websocket server which responds to JSONRPC frames, where `method` is:
    /// `[agent_key]/[dna_hash]/[zome_name]/[trait_name]/[function_name]`
    /// and `params` is a `serde_json::Value`
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

    // #[test]
    // fn can_start_server() {
    //     let (agents, dnas, holochain_map) = mock_data(vec!["a1", "a2"]);
    //     let handle = thread::spawn(move || {
    //         HcWebsocketRpcServer::new()
    //             .with_holochains(holochain_map)
    //             .start_holochains()
    //             .unwrap()
    //             .serve("4321")
    //             .unwrap();
    //     });
    //     handle.thread().unpark();
    // }

    // #[test]
    // fn can_call_holochains() {
    //     let (agents, dnas, holochain_map) = mock_data(vec!["a3", "a4"]);
    //     let server = HcWebsocketRpcServer::new().with_holochains(holochain_map);
    //     server.start_holochains().unwrap();

    //     let method = format!(
    //         "{}/{}/blog/main/create_post",
    //         agents[0].to_string(),
    //         util::get_dna_hash(&dnas[0]),
    //     );
    //     let params = json!({
    //         "content": "i see you",
    //         "in_reply_to": "the moon",
    //     });
    //     let result = server.call_rpc(&method, params).unwrap();
    //     println!("TODO - check once this is not an error: {:?}", result);
    // }
}
