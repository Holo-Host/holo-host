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
    version: String,
    method: String,
    params: Value,
    id: String,
}

fn parse_jsonrpc(s: &str) -> Result<JsonRpc, String> {
    let msg: JsonRpc = serde_json::from_str(s).map_err(|e| e.to_string())?;
    if msg.version != "2.0" {
        Err("JSONRPC version must be 2.0".to_string())
    } else {
        Ok(msg)
    }
}

fn invoke_call(holochains: &mut HcDex, rpc_method: &str, params: Value) -> Result<Value, HolochainError> {
    let matches: Vec<&str> = rpc_method.split('/').collect();
    let result = if let [agent, dna, zome, cap, func] = matches.as_slice() {
        let mut hc = holochains.get_mut(&(agent.to_string(), dna.to_string())).expect("No instance for agent/dna pair");
        hc.call(zome, cap, func, &params.to_string()).map(Value::from).map_err(|e| e.to_string())
    } else {
        Err("bad rpc method".to_string())
    };
    result.map_err(HolochainError::ErrorGeneric)
}

pub type HcDex = HashMap<(String, String), Holochain>;

struct RpcHandler();

pub fn start_ws_server(
    port: &str,
    holochains: HcDex,
) -> WsResult<()> {
    ws::listen(format!("localhost:{}", port), |out| {
        move |msg| {
            match msg {
                Message::Text(s) => {
                    let rpc: JsonRpc = parse_jsonrpc(s.as_str()).unwrap();
                    let response = invoke_call(&mut holochains, rpc.method.as_str(), rpc.params).unwrap();
                    out.send(Message::Text(response.to_string()))
                },
                Message::Binary(b) => Ok(())
            }
        }
    })
}

fn rpc_method_name(dna_name:&String, zome_name:&String, cap_name:&String, func_name:&String) -> String {
    format!("{}/{}/{}/{}", dna_name, zome_name, cap_name, func_name)
}


// pub fn main() {
// 	let mut io = MetaIoHandler::with_middleware(MyMiddleware::default());
//
// 	io.add_method_with_meta("say_hello", |_params: Params, meta: Meta| {
// 		Ok(Value::String(format!("Hello World: {}", meta.0)))
// 	});
//
// 	let request = r#"{"jsonrpc": "2.0", "method": "say_hello", "params": [42, 23], "id": 1}"#;
// 	let response = r#"{"jsonrpc":"2.0","result":"Hello World: 5","id":1}"#;
//
// 	let headers = 5;
// 	assert_eq!(
// 		io.handle_request(request, Meta(headers)).wait().unwrap(),
// 		Some(response.to_owned())
// 	);
// }

// fn rpc_method<'a, 'b>(
//     hc: &'b mut Holochain,
//     dna_name: String,
//     zome_name: String,
//     cap_name: String,
//     func_name: String
// ) -> Callback<'a> {
//     Box::new(|params| {
//         hc.call(
//             zome_name.as_str(),
//             cap_name.as_str(),
//             func_name.as_str(),
//             params.parse::<Value>().unwrap().to_string().as_str(),
//         )
//             .map_err(|e| WsError::from( e.to_string() ))
//             .and_then(|r| serde_json::from_str(r.as_str()).map_err(|e| WsError::from(e.to_string())))

//     })
// }


    // for dna in dnas {
    //     let dna_name = dna.name;
    //     for (zome_name, zome) in dna.zomes {
    //         for (cap_name, cap) in zome.capabilities {
    //             match cap.cap_type.membrane {
    //                 Membrane::Public | Membrane::Agent => {
    //                     for func in cap.functions {
    //                         let func_name = func.name;
    //                         let method_name = rpc_method_name(&dna_name, &zome_name, &cap_name, &func_name);
    //                         // let method = rpc_method(
    //                         //     hc, dna_name, zome_name, cap_name, func_name
    //                         // );
    //                         // let mk_err = |code: ErrorCode| move |e: WsError| Error {
    //                         //     code, message: e.to_string(), data: None
    //                         // };


    //                         let method = |params: Params| -> Result<Value, Error> {
    //                             Ok(Value::from(r#""hi""#));
    //                             hc.call(
    //                                 zome_name.as_str(),
    //                                 cap_name.as_str(),
    //                                 func_name.as_str(),
    //                                 params.parse::<Value>().unwrap().to_string().as_str(),
    //                             )
    //                             .map_err(|e| Error {
    //                                 code: ErrorCode::ParseError,
    //                                 message: e.to_string(),
    //                                 data: None,
    //                             })
    //                             .and_then(|r| serde_json::from_str(r.as_str())
    //                                 .map_err(|e| Error {
    //                                     code: ErrorCode::ParseError,
    //                                     message: e.to_string(),
    //                                     data: None,
    //                                 })
    //                             )
    //                         };
    //                         println!("{}", method_name);
    //                         io.add_method(method_name.as_str(), method);
    //                     }
    //                 },
    //                 _ => ()
    //             }
    //         }
    //     }
    // }
