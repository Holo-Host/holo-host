use bson::oid::ObjectId;
use clap::Subcommand;
use db_utils::schemas::workload::WorkloadManifestHolochainDhtV1;
use nats_utils::types::{HcHttpGwRequest, NatsRemoteArgs};

#[derive(Clone, clap::Parser)]
pub struct RemoteArgs {
    #[clap(flatten)]
    pub nats_remote_args: NatsRemoteArgs,

    #[arg(
        long,
        default_value_t = false,
        help = "don't wait for Ctrl+C being pressed before exiting the process",
        env = "DONT_WAIT"
    )]
    pub dont_wait: bool,
}

/// A set of commands for remotely interacting with a running host-agent instance, by exchanging NATS messages.
#[derive(Subcommand, Clone)]
pub enum RemoteCommands {
    /// Status
    Ping {},

    /// Manage workloads.
    HolochainDhtV1Workload {
        #[arg(long)]
        workload_id_override: Option<ObjectId>,

        // currently used for the publish subject in case we forego the orchestrator
        #[arg(long)]
        host_id: Option<String>,

        #[arg(long)]
        desired_status: String,

        #[command(flatten)]
        manifest: Box<WorkloadManifestHolochainDhtV1>,

        #[arg(long)]
        workload_only: bool,

        #[arg(long)]
        subject_override: Option<String>,

        #[arg(long, default_value = "WORKLOAD.>")]
        /// If provided, the CLI will subscribe to the given subject on the remote NATS after publishing the workload message.
        maybe_wait_on_subject: Option<String>,
    },

    HcHttpGwReq {
        #[clap(flatten)]
        request: HcHttpGwRequest,
    },
}
