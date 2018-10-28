use std::collections::HashMap;
use std::slice::IterMut;
// use std::sync::mpsc::{channel, Sender, Receiver};

use jsonrpc_ws_server::{
    Error as WsError,
    Result as WsResult,
    Server,
    ServerBuilder,
    jsonrpc_core::{
        MetaIoHandler,
        Error,
        ErrorCode,
        types::{
            Params,
        }
    }
};
use serde_json::Value;

use holochain_core::{
    context::Context,
    logger::SimpleLogger,
    persister::SimplePersister,
};
use holochain_core_api::Holochain;
use holochain_dna::{
    zome::capabilities::Membrane,
    Dna,
};

type RpcSegments = (String, String, String, String);

pub fn rpc_tuples(dna: Dna) -> Vec<RpcSegments> {
    let mut tuples = Vec::new();
    for (zome_name, zome) in dna.zomes {
        for (cap_name, cap) in zome.capabilities {
            match cap.cap_type.membrane {
                Membrane::Public | Membrane::Agent => {
                    for func in cap.functions {
                        tuples.push(
                            (dna.name, zome_name, cap_name, func.name)
                        );
                    }
                },
                _ => ()
            }
        }
    }
    tuples
}

pub fn start_ws_server<S: Iterator<Item=Dna>>(
    port: &str,
    holochains: Vec<(Holochain, Dna)>,
) -> WsResult<Server> {
    let xyzzy = holochains.iter_mut().map(|(hc, dna)| {
        let segments = rpc_tuples(dna.clone());
        (segments, hc)
    });
    let middleware = MyMiddleware::new(xyzzy.collect());
    let mut io = MetaIoHandler::with_middleware(middleware);
    
    let socket_addr = format!("0.0.0.0:{}", port);
    ServerBuilder::new(io).start(&socket_addr.as_str().parse().unwrap())
}

fn rpc_method_name(dna_name:&String, zome_name:&String, cap_name:&String, func_name:&String) -> String {
    format!("{}/{}/{}/{}", dna_name, zome_name, cap_name, func_name)
}



use std::time::Instant;
use std::sync::atomic::{self, AtomicUsize};

use jsonrpc_ws_server::{
    jsonrpc_core::{Metadata, Middleware, FutureResponse, Request, Response},
    jsonrpc_core::futures::Future,
    jsonrpc_core::futures::future::Either,
};


type HcArgs = (Dna, Context);

// https://gist.github.com/aisamanra/da7cdde67fc3dfee00d3
type Callback<'a> = Box<(FnMut(&'a Params) -> WsResult<Value>) + 'static>;

#[derive(Clone)]
struct Meta(usize);
impl Metadata for Meta {}

#[derive(Default)]
struct MyMiddleware<'a> {
    number: AtomicUsize,
    holochains: HashMap<String, (RpcSegments, &'a mut Holochain)>
}

impl<'a> MyMiddleware<'a> {
    pub fn new<I>(hcs: Vec<(Vec<RpcSegments>, &'a mut Holochain)>) -> Self 
        
    {
        let mut inst = Self::default();
        inst.holochains = hcs.iter_mut().map(|&mut pair| {
            let (segments, _) = pair;
            let (d, z, c, f) = segments;
            let name = rpc_method_name(&d, &z, &c, &f);
            (name, pair)
        }).collect();
        inst
    }
}

// https://github.com/paritytech/jsonrpc/blob/master/core/examples/middlewares.rs
impl Middleware<Meta> for MyMiddleware<'static> {
	type Future = FutureResponse;

	fn on_request<F, X>(&self, request: Request, meta: Meta, next: F) -> Either<Self::Future, X> where
		F: FnOnce(Request, Meta) -> X + Send,
		X: Future<Item=Option<Response>, Error=()> + Send + 'static,
	{
		let start = Instant::now();
		let request_number = self.number.fetch_add(1, atomic::Ordering::SeqCst);
		println!("Processing request {}: {:?}", request_number, request);

		Either::A(Box::new(next(request, meta).map(move |res| {
			println!("Processing took: {:?}", start.elapsed());
			res
		})))
	}
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