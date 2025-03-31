use db_utils::schemas::{DATABASE_NAME, PUBLIC_SERVICE_COLLECTION_NAME};
use holo_dns::{dns_cache, dns_service};
use log::info;
use permit::Permit;
use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
use signal_hook::iterator::Signals;
use std::env;

/// The TCP/UDP ports to listen on. By default, we listen on port 53, but for testing, we want to
/// use a different port number to avoid conflicts with systemd and other local caching
/// resolvers. Port 53 is also <1024, which requires us to be root to bind to the socket. This
/// allows tests to run as non-root.
const DNS_SERVER_PORT: &str = match option_env!("DNS_SERVER_PORT") {
    Some(v) => v,
    None => "53",
};

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting Holo-DNS version {}", env!["CARGO_PKG_VERSION"]);

    let top_permit = Permit::new();
    let permit = top_permit.new_sub();

    // Start signal handler thread for clean exit
    tokio::spawn(async move {
        Signals::new([SIGHUP, SIGQUIT, SIGINT, SIGTERM])
            .unwrap()
            .forever()
            .next();

        info!("Shutting down");

        drop(top_permit);
    });

    // determine which source to use for DNS cache records
    let db_uri_result = env::var("DB_URI");
    let cache_source: dns_cache::DnsCacheSource = match db_uri_result {
        Ok(db_uri) => {
            let db_name = match env::var("DB_NAME") {
                Ok(db_name) => db_name,
                Err(_) => {
                    // Default database name if not specified
                    DATABASE_NAME.to_string()
                }
            };
            let db_collection = match env::var("DB_COLLECTION") {
                Ok(db_collection) => db_collection,
                Err(_) => {
                    // Default database name if not specified
                    PUBLIC_SERVICE_COLLECTION_NAME.to_string()
                }
            };
            dns_cache::DnsCacheSource::MongoDb(dns_cache::MongoDbSourceParms {
                db_uri,
                db_name,
                db_collection,
            })
        }
        Err(_) => {
            let json_file = match env::var("JSON_FILE") {
                Ok(file) => file,
                Err(_) => panic!("If not using DB_URI source, specify JSON_FILE"),
            };
            dns_cache::DnsCacheSource::JsonFile(dns_cache::JsonFileSourceParms { json_file })
        }
    };

    // Thread to run in the background and periodically update our cache of names/addresses.
    info!("Starting DNS cache update thread.");
    dns_cache::start_cache(cache_source).await;

    // Start the DNS listener.
    let port_num: u16 = DNS_SERVER_PORT
        .parse::<u16>()
        .expect("DNS_SERVER_PORT environment variable cannot be parsed to a u16.");
    info!("Starting DNS service listener on port {}.", port_num);
    dns_service::start_service(port_num, &permit);
}
