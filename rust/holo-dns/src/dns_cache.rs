use db_utils::mongodb::{MongoCollection, MongoDbAPI};
use db_utils::schemas::PublicService;
use log::{debug, info};
use mongodb::bson::doc;
use mongodb::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;
use thiserror::Error;
use tokio::spawn;

#[derive(Debug, Error)]
enum CacheError {
    #[error("NATS DB Error")]
    NatsError(#[from] nats_utils::types::ServiceError),
    #[error("Infallible Error")]
    InfallibleError,
}

/// A lot of MongoDB calls return infallible, which thiserror doesn't directly support.
impl From<std::convert::Infallible> for CacheError {
    fn from(_: std::convert::Infallible) -> Self {
        CacheError::InfallibleError
    }
}

/// The delay in between cache refreshes. The current set of DNS records is going to be very small
/// and pulling a new copy will be cheap. A 60 second delay just means that after a new workload is
/// created, there may be a maximum of 60 seconds before it resolves.
///
/// We primarily allow this to be overridden by environment variable for tests, but it could be
/// used to fine-tune things in production too.
const DNS_CACHE_UPDATE_TIME: &str = match option_env!("DNS_CACHE_UPDATE_TIME") {
    Some(v) => v,
    None => "60",
};

/// Global cache, behind reader-writer lock. The HashMap key is a string representation of the name
/// that we're being asked to look up. The value is a struct containing all of the A and AAAA
/// answers for that query. This will be incredibly small until we start having to host many, many
/// public-IP services.
pub static DNS_CACHE: LazyLock<Arc<RwLock<HashMap<String, DnsCacheItem>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

/// DnsCacheItem just represents a set of cached answers for a given query on the [DNS_CACHE]
/// cache. The cache itself is a HashMap of [DnsCacheItem]s with the key as an FQDN. We're
/// deliberately conflating all RRs under a particular FQDN, and not enforcing some DNS strictness
/// -- the dns-server crate enforces the right data type and we should make sure that the cache
/// input is sane. Being flexible here gives us more options when it comes to testing and other
/// nefarious deeds in the future.
///
/// The primary outcome of this is that the vectors below could be empty for any given record type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsCacheItem {
    pub a: Vec<String>,
    pub aaaa: Vec<String>,
    pub cname: Vec<String>,
    pub ns: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct JsonFileSourceParms {
    pub json_file: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MongoDbSourceParms {
    pub db_uri: String,
    pub db_name: String,
    pub db_collection: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DnsCacheSource {
    MongoDb(MongoDbSourceParms),
    JsonFile(JsonFileSourceParms),
}

pub async fn start_cache(source: DnsCacheSource) {
    if let DnsCacheSource::JsonFile(j) = source {
        info!("Selected JSON file source.");
        spawn(async move { start_json_cache(j).await });
    } else if let DnsCacheSource::MongoDb(m) = source {
        info!("Selected MongoDB source.");
        spawn(async move { start_mongodb_cache(m).await });
    }
}

/// Inner function to poll the MongoDB cache. We don't want to ignore errors, but also don't want
/// the cache to fail in case of temporary issues. This allows us to catch and log a variety of
/// errors and keep the outer function and thread trying.
async fn mongo_db_inner(input: &MongoDbSourceParms) -> Result<(), CacheError> {
    // Using unwrap() on the next few lines doesn't seem right on the face of it, but they're
    // Infallible and MongoDB doesn't actually try to connect until we actually try to query
    // further down. This will fail if the URL cannot be parsed, at which point we have no choice
    // but to bail.
    let client = Client::with_uri_str(&input.db_uri).await.unwrap();
    let collection =
        MongoCollection::<PublicService>::new(&client, &input.db_name, &input.db_collection)
            .await
            .unwrap();
    let filter = doc! { "service_type": "GatewayServer" };
    let gateway_nodes = collection.get_one_from(filter).await;
    match gateway_nodes {
        Ok(ret) => {
            // Success, parse and update cache.
            match ret {
                Some(service) => {
                    // Take write lock, update cache and we're done.
                    {
                        let mut cache = DNS_CACHE.write().unwrap();
                        cache.clear();
                        cache.insert(
                            service.service_name,
                            DnsCacheItem {
                                a: service.a_addrs.clone(),
                                aaaa: service.aaaa_addrs.clone(),
                                cname: service.cname_addrs.clone(),
                                ns: service.ns_addrs.clone(),
                            },
                        );
                    }
                }
                None => {
                    info!("MongoDB returned no records for GatewayServer type.");
                }
            }
        }
        Err(e) => {
            // Leave the cache intact in case the records aren't stale, and wiaut for
            // the next loop around.
            info!("MongoDB query for GatewayServer doc failed: {}", e);
            return Err(CacheError::NatsError(e));
        }
    }
    Ok(())
}

async fn start_mongodb_cache(input: MongoDbSourceParms) {
    let mut delay = 0;
    let configured_delay: u64 = DNS_CACHE_UPDATE_TIME
        .parse::<u64>()
        .expect("DNS_CACHE_UPDATE_TIME environment variable cannot be parsed into a u64.");
    // TODO: Remove this before merging -- URI containers the user's password....
    info!(
        "Starting DNS cache with MongoDB source using URL {}.",
        input.db_uri,
    );
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(delay)) => {
                debug!("Updating DNS cache from MongoDB backend");
                match mongo_db_inner(&input).await {
                    Ok(_) => debug!("Repopulated cache from MongoDB."),
                    Err(e) => info!("Failed to repopulate cache from MongoDB: {}", e),
                }
                delay = configured_delay;
            },
        }
    }
}

async fn start_json_cache(input: JsonFileSourceParms) {
    debug!("In JSON cache");
    let mut delay = 0;
    let configured_delay: u64 = DNS_CACHE_UPDATE_TIME
        .parse::<u64>()
        .expect("DNS_CACHE_UPDATE_TIME environment variable cannot be parsed into a u64.");
    info!(
        "Starting DNS cache with JSON file {} as data source.",
        input.json_file
    );
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(delay)) => {
                // If the file they gave us can't be opened, we can't do much other than bail.
                let file = File::open(&input.json_file).unwrap();
                let reader = BufReader::new(file);
                let dns_data: HashMap<String, DnsCacheItem> = serde_json::from_reader(reader).unwrap();
                debug!("Read DNS data: {:?}", dns_data);
                // Scope for holding the write lock below.
                {
                    let mut cache = DNS_CACHE.write().unwrap();
                    cache.clear();
                    for key in dns_data.keys() {
                        cache.insert(key.to_string(), dns_data[key].clone());
                    }
                }

                delay = configured_delay;
            }
        }
    }
}
