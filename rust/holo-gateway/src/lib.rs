pub mod routes;
pub mod types;

use anyhow::Result;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use nats_utils::jetstream_client::JsClient;
use nats_utils::types::NatsRemoteArgs;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use uuid::Uuid;

#[cfg(feature = "broken")]
#[cfg(test)]
mod tests;

#[derive(clap::Parser, Debug)]
pub struct RunArgs {
    #[arg(
        long,
        env = "NODE_ID",
        help = "UUID, defaults to generating a random one.",
        default_value_t = Uuid::new_v4(),
    )]
    pub node_id: Uuid,

    #[arg(long, env = "LISTEN", default_value = "0.0.0.0:8000")]
    pub listen: SocketAddr,

    #[clap(flatten)]
    pub nats_remote_args: NatsRemoteArgs,
}

pub impl RunArgs {
    pub fn populate_from_environment() -> Self {
        let password_file = env::_var("NATS_PASSWORD_FILE").expect("NATS_PASSWORD_FILE must be set");
        let password = env::_var("NATS_PASSWORD").expect("NATS_PASSWORD must be set");
        let username = env::_var("NATS_USERNAME").expect("NATS_USERNAME must be set");
        let nats_url = env::_var("NATS_URL").expect("NATS_URL must be set");
        holo_gateway::RunArgs {
            node_id: Uuid::new_v4(),
            listen: "0.0.0.0:8000",
            nats_remote_args: nats_utils::types::NatsRemoteArgs {
                nats_url,
                nats_user: Some(username),
                nats_password: Some(password),
                nats_password_file: Some(password_file),
                nats_skip_tls_verification_danger: false
            },
        }
    }
}

pub async fn run(nats_client: Arc<JsClient>, args: RunArgs) -> Result<()> {
    let listener = TcpListener::bind(args.listen).await?;

    log::info!("Listening on {} as node ID: {}", args.listen, args.node_id,);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let nats_client = Arc::clone(&nats_client);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(
                    io,
                    service_fn(|req| {
                        routes::http_router(&args.node_id, Arc::clone(&nats_client), req)
                    }),
                )
                .with_upgrades()
                .await
            {
                log::error!("Failed to service connection: {err:?}");
            }
        });
    }
}
