/*
  This file contains the configuration required to set up a NATS Leaf Server with the "Operator JWT" auth approach.
  NB: This setup expects the `nats-server` binary to be locally installed and accessible.
*/
use std::fs::File;
use std::io::Write;
use std::process::Command;

#[derive(Debug)]
pub struct JetStreamConfig {
    store_dir: String,
    max_memory_store: u64,
    max_file_store: u64,
}

#[derive(Debug)]
pub struct LoggingOptions {
    debug: bool,
    trace: bool,
    longtime: bool,
}

#[derive(Debug)]
pub struct LeafNodeRemote {
    url: String,
    credentials_path: String,
    account_key: String,
}

#[derive(Debug)]
pub struct LeafServer {
    name: String,
    host: String,
    port: u16,
    jetstream_config: JetStreamConfig,
    operator_path: String,
    system_account_key: String,
    logging: LoggingOptions,
    leaf_nodes_remote: Vec<LeafNodeRemote>,
    resolver_path: String,
}

impl LeafServer {
    // Instantiate a new leaf server
    pub fn new(
        server_name: &str,
        host: &str,
        port: u16,
        jetstream_config: JetStreamConfig,
        operator_path: &str,
        system_account_key: &str,
        logging: LoggingOptions,
        leaf_nodes_remote: Vec<LeafNodeRemote>,
        resolver_path: &str,
    ) -> Self {
        Self {
            name: server_name.to_string(),
            host: host.to_string(),
            port,
            jetstream_config,
            operator_path: operator_path.to_string(),
            system_account_key: system_account_key.to_string(),
            logging,
            leaf_nodes_remote,
            resolver_path: resolver_path.to_string(),
        }
    }

    /// Generate the config file and run the server
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = "./leaf_server.conf";
        let mut config_file = File::create(config_path)?;

        // Generate logging options
        let logging_config = format!(
            "debug: {}\ntrace: {}\nlogtime: {}\n",
            self.logging.debug, self.logging.trace, self.logging.longtime
        );

        // Generate the "leafnodes" block
        let leafnodes_config = self
            .leaf_nodes_remote
            .iter()
            .map(|leaf| {
                format!(
                    r#"
                {{
                    url: "{}",
                    credentials: "{}",
                    account: "{}"
                }}
                "#,
                    leaf.url, leaf.credentials_path, leaf.account_key
                )
            })
            .collect::<Vec<String>>()
            .join(",\n");

        // Write the full config file
        write!(
            config_file,
            r#"
server_name: {}
listen: "{}:{}"

operator: "{}"
system_account: "{}"

jetstream: {{
    domain: "leaf",
    store_dir: "{}",
    max_mem: {},
    max_file: {}
}}

leafnodes: {{
    remotes: [
        {}
    ]
}}

includes {}

logging: {{
    {}
}}
"#,
            self.name,
            self.host,
            self.port,
            self.operator_path,
            self.system_account_key,
            self.jetstream_config.store_dir,
            self.jetstream_config.max_memory_store,
            self.jetstream_config.max_file_store,
            leafnodes_config,
            self.resolver_path,
            logging_config
        )?;

        // Run the server with the generated config
        let mut child = Command::new("nats-server")
            .arg("-c")
            .arg(config_path)
            .spawn()
            .expect("Failed to start NATS server");

        println!("NATS Leaf Server is running at {}:{}", self.host, self.port);

        // Wait for the server to finish and print a log when so...
        let status = child.wait()?;
        println!("NATS server exited with status: {:?}", status);

        Ok(())
    }
}
