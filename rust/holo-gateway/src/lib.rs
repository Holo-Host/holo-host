pub mod nats_client;
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
