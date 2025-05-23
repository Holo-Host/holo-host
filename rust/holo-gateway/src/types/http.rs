/// This module contains data structures and types representing forwarded/proxied requests and
/// responses.
use async_nats::header::{
    HeaderMap as NatsHeaderMap, HeaderName as NatsHeaderName, HeaderValue as NatsHeaderValue,
};
use hyper::header::HeaderMap as HyperHeaderMap;
use hyper::{body::Body, Method, Request};
use nats_utils::types::HcHttpGwRequest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use url_parse::core::Parser;
use uuid::Uuid;
use workload::WORKLOAD_SRV_SUBJ;

use crate::types::nats::HTTP_GW_SUBJECT_NAME;

use super::error::HoloHttpGatewayError;

/// Static DNS hostname for holochain gateway nodes. TODO: Wrap in environment variable for
/// override. Also pending discussion with holochain team.
//const HC_GATEWAY_HOSTNAME: &str = "gw.dna.holo.host";
pub fn get_holo_gw_host() -> String {
    std::env::var("HC_GATEWAY_HOSTNAME").unwrap_or_else(|_| "localhost:8000".to_string())
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
    HolochainHTTP(HcHttpGwRequest),
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
    pub dna_hash: String,
    /// Instance ID of the coordinator that is running this instance?
    pub coordinator_id: String,
    /// The name of the holochain zome within the hApp
    pub zome_name: String,
    /// Function to call within the Holochain zome
    pub function_name: String,
    /// base64url-encoded JSON payload for the zome call
    pub payload: String,
}
impl HolochainHTTP {
    pub fn nats_request_subject(&self) -> String {
        format!(
            "{WORKLOAD_SRV_SUBJ}.{HTTP_GW_SUBJECT_NAME}.{}",
            self.coordinator_id
        )
    }
    pub fn nats_reply_subject(&self) -> String {
        format!("{}.reply", self.nats_request_subject())
    }
}

/// A serde-compatible structure for passing around a HTTP request.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ForwardedHTTPRequest {
    /// The HTTP method (eg, GET)
    method: HTTPMethod,
    /// URI path+params portion (eg, /dna/id:xxxxyyyy?call_name=something&param1=something_else).
    uri: String,
    /// A selection of HTTP headers passed in by the client, and some added by us.
    pub headers: NatsHeaderMap,
    /// The request body
    body: Vec<u8>,
    /// Potential protocol implemented on top of HTTP, such as Holochain HTTP Gateway.
    pub super_proto: Option<SuperProtocol>,
}

// Converts a `hyper::HeaderMap` to an `async_nats::HeaderMap`
pub fn try_into_nats_headers(
    hyper_headers: &HyperHeaderMap,
) -> Result<NatsHeaderMap, HoloHttpGatewayError> {
    let mut nats_headers = NatsHeaderMap::new();

    for (key, value) in hyper_headers.iter() {
        if let Ok(value_str) = value.to_str() {
            nats_headers.insert(key.as_str(), NatsHeaderValue::from_str(value_str)?);
        } else {
            log::error!(
                "Failed to convert Hyper header value to Nats header value: {:?}",
                value
            );
        }
    }
    Ok(nats_headers)
}

impl ForwardedHTTPRequest {
    /// Given a Request object from hyper, construct our own request. In the case of most
    /// attributes, it's just a one-for-one copy, but we'll also use this to add a few other
    /// headers for tracking/diagnostics, and potentially in the future add/filter parts of
    /// requests.
    pub fn from_hyper<B>(req: &Request<B>, node_id: &Uuid) -> Self
    where
        B: Body + Send + Sync + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        // Parse the path and query string portion to determine whether this is a holochain request
        // or not.
        let uri = Parser::new(None).parse(&req.uri().to_string()).unwrap();
        log::info!("uri: {:?}", &uri);

        let super_proto = Self::is_super(req);

        let headers = Self::handle_headers(req.headers(), node_id).unwrap_or_default();

        ForwardedHTTPRequest {
            method: Self::method(req.method()),
            uri: req.uri().to_string(),
            body: vec![], // TODO: add body
            headers,
            super_proto,
        }
    }

    fn handle_headers(
        hyper_headers: &HyperHeaderMap,
        node_id: &Uuid,
    ) -> Result<NatsHeaderMap, HoloHttpGatewayError> {
        // We'll likely want to filter certain headers out at some point, but for now, we'll just
        // pass all headers through.
        let mut nats_headers: NatsHeaderMap = try_into_nats_headers(hyper_headers)?;

        // Insert a header to uniquely identify this request and the response that comes back. This
        // will be helpful in debugging and also in analytics later on.
        let request_id = Uuid::new_v4();
        nats_headers.insert(
            NatsHeaderName::from_str("X-Holo-RequestID")?,
            NatsHeaderValue::from_str(&request_id.to_string())?,
        );
        nats_headers.insert(
            NatsHeaderName::from_str("X-Holo-ForwarderID")?,
            NatsHeaderValue::from(node_id.to_string()),
        );

        Ok(nats_headers)
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
    fn is_super<B>(req: &Request<B>) -> Option<SuperProtocol>
    where
        B: Body + Send + Sync + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        // TODO: remove unwrap()s
        let uri = Parser::new(None).parse(&req.uri().to_string()).unwrap();

        let fqdn = req.headers()["host"].to_str().unwrap().to_string();
        let parts: Vec<&str> = fqdn.split('.').collect();
        // first part is the hostname
        let _hostname = parts.first().unwrap().to_string();
        // the rest is the domain name
        let _domain = parts.join(".");

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

        // These may need adjusting, depending on feedback from the Holochain team and integration testing.
        // TODO: clarify hostname/domain requirements hostname == get_holo_gw_host() &&
        if req.method() == Method::GET && path_len == 4 {
            return Some(SuperProtocol::HolochainHTTP(HcHttpGwRequest {
                dna_hash: path_components[0].clone(),
                coordinatior_identifier: path_components[1].clone(),
                zome_name: path_components[2].clone(),
                zome_fn_name: path_components[3].clone(),
                payload,
            }));
        }

        None
    }
}
