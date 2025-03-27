/// This module contains data structures and types representing forwarded/proxied requests and
/// responses.
use hyper::http::{Method, Request};
use log::info;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use url_parse::core::Parser;
use uuid::Uuid;

/// Static DNS hostname for holochain gateway nodes. TODO: Wrap in environment variable for
/// override. Also pending discussion with holochain team.
//const HC_GATEWAY_HOSTNAME: &str = "gw.dna.holo.host";
const HC_GATEWAY_HOSTNAME: &str = "localhost:8000";

/// A serde-compatible structure for passing around a HTTP request.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ForwardedHTTPRequest {
    /// The HTTP method (eg, GET)
    method: HTTPMethod,
    /// URI path+params portion (eg, /dna/id:xxxxyyyy?call_name=something&param1=something_else).
    uri: String,
    /// A selection of HTTP headers passed in by the client, and some added by us.
    headers: HashMap<String, Vec<u8>>,
    /// The request body
    body: Vec<u8>,
    /// Potential protocol implemented on top of HTTP, such as Holochain HTTP Gateway.
    super_proto: Option<SuperProtocol>,
}

impl ForwardedHTTPRequest {
    /// Given a Request object from hyper, construct our own request. In the case of most
    /// attributes, it's just a one-for-one copy, but we'll also use this to add a few other
    /// headers for tracking/diagnostics, and potentially in the future add/filter parts of
    /// requests.
    pub fn from_hyper(req: &Request<hyper::body::Incoming>, node_id: &str) -> ForwardedHTTPRequest {
        // We'll likely want to filter certain headers out at some point, but for now, we'll just
        // pass all headers through.
        let mut headers: HashMap<String, Vec<u8>> = HashMap::new();
        for (k, v) in req.headers().into_iter() {
            headers.insert(k.to_string(), v.as_bytes().to_vec());
        }

        // Parse the path and query string portion to determine whether this is a holochain request
        // or not.
        let uri = Parser::new(None).parse(&req.uri().to_string()).unwrap();
        info!("uri: {:?}", &uri);

        let super_proto = Self::is_super(req);

        // Insert a header to uniquely identify this request and the response that comes back. This
        // will be helpful in debugging and also in analytics later on.
        let request_id = Uuid::new_v4();
        headers.insert("X-Holo-RequestID".to_string(), request_id.into());
        headers.insert("X-Holo-ForwarderID".to_string(), node_id.into());
        ForwardedHTTPRequest {
            method: Self::method(req.method()),
            uri: req.uri().to_string(),
            body: vec![], // TODO: add body
            headers,
            super_proto,
        }
    }

    fn method(method: &Method) -> HTTPMethod {
        match *method {
            Method::GET => HTTPMethod::Get,
            Method::PUT => HTTPMethod::Put,
            Method::POST => HTTPMethod::Post,
            Method::DELETE => HTTPMethod::Delete,
            _ => HTTPMethod::Unsupported,
        }
    }

    /// We only currently support one protocol over HTTP(S) -- the protocol of the Holchain web
    /// gateway. It has specific context and meaning around specific path components and
    /// parameters. This function could be split out to support others later, but for now just
    /// parses enough out of the HC GW request to allow us to successfully route the request to the
    /// right place and get a response back.
    fn is_super(req: &Request<hyper::body::Incoming>) -> Option<SuperProtocol> {
        // TODO: remove unwrap()s
        let uri = Parser::new(None).parse(&req.uri().to_string()).unwrap();

        let fqdn = req.headers()["host"].to_str().unwrap().to_string();
        let parts: Vec<&str> = fqdn.split('.').collect();
        // first part is the hostname
        let hostname = parts.first().unwrap().to_string();
        // the rest is the domain name
        let domain = parts.join(".");

        let mut payload = "".to_string();
        // parse out the parameters, even though we don't use them yet.
        let mut _params: HashMap<String, String> = HashMap::new();
        if let Some(query_string) = uri.query {
            for parm in query_string.split("&") {
                if let Some((k, v)) = parm.split_once("=") {
                    if k == "payload" {
                        // parse the payload parameter out -- it's the part of the interface to the
                        // holochain gateway.
                        payload = v.to_string();
                    }
                    _params.insert(k.to_string(), v.to_string());
                }
            }
        }

        // Split the URL path up into components determined by the HC gateway.
        let mut path_len = 0;
        let mut path_components = vec![];
        if let Some(path) = uri.path {
            path_len = path.len();
            if path_len != 4 {
                // Unless we have exactly 4 path components, this isn't for the holochain gateway.
                return None;
            }
            // coordinator_id
            path_components.push(path[0].clone());
            // coordinator_id
            path_components.push(path[1].clone());
            // zome_name
            path_components.push(path[2].clone());
            // function_name
            path_components.push(path[3].clone());
        }

        // These may need adjusting, depending on feedback from the Holochain team and integration
        // testing.
        if hostname == HC_GATEWAY_HOSTNAME && req.method() == Method::GET && path_len == 4 {
            return Some(SuperProtocol::HolochainHTTP(HolochainHTTP {
                hostname,
                domain,
                dna_hash: path_components[0].clone(),
                coordinator_id: path_components[1].clone(),
                zome_name: path_components[2].clone(),
                function_name: path_components[3].clone(),
                payload,
            }));
        }

        None
    }
}

/// The HTTP request method being used. Currently we only deal with GET requests.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum HTTPMethod {
    Get,
    Put,
    Post,
    Delete,
    Unsupported,
}

/// It's possible for protocols to be implemented on top of the HTTP protocol and we may want to
/// use information within those protocols to aid in routing traffic to the right place. One
/// example of this is the Holochain HTTP protocol, which encodes URLs and hostnames in a
/// particular way, so as to allow generic code to query the contents of a DHT via a DNA.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SuperProtocol {
    HolochainHTTP(HolochainHTTP),
}

/// Support for parts of the Holochain-over-HTTP protocol that we'll use to route the traffic to
/// the correct place.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HolochainHTTP {
    /// The hostname contains that Holo Hosting instance/job ID
    hostname: String,
    /// The domain portion of the hostname, minus the host portion
    domain: String,
    /// DNA hash
    dna_hash: String,
    /// Instance ID of the coordinator that is running this instance?
    coordinator_id: String,
    /// The name of the holochain zome within the hApp
    zome_name: String,
    /// Function to call within the Holochain zome
    function_name: String,
    /// base64url-encoded JSON payload for the zome call
    payload: String,
}
