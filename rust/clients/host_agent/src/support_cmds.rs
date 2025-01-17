use crate::agent_cli::SupportCommands;
use netdiag::*;

pub fn support_command(command: &SupportCommands) -> Result<(), std::io::Error> {
    // TODO: Fill these in under a separate set of commits to keep PRs simple.
    match command {
        SupportCommands::NetTest {
            nameserver,
            ip_version,
            hostname,
            use_tls,
            port,
            http_path,
        } => {
            println!(
                "Query host: {}; port: {}; path: {}; Name server: {}",
                hostname, port, http_path, nameserver
            );
            let cfg = NetDiagConfig {
                global: GlobalConfig {
                    ip_version: ip_version.clone(),
                },
                queries: vec![QueryDefinition {
                    nameserver: nameserver.to_string(),
                    use_tls: *use_tls,
                    hostname: hostname.to_string(),
                    port: *port,
                    method: QueryMethod::HTTP {
                        path: http_path.to_string(),
                    },
                }],
            };

            let stats = do_requests(&cfg);
            for stat in stats.iter() {
                match &stat.result {
                    Ok(_) => {
                        println!("Request succeeded");
                    }
                    Err(e) => {
                        println!("Request failed with: {}", e);
                    }
                }
                for phase in &stat.stats {
                    let result = match &phase.error {
                        None => {
                            format!("succeeded")
                        }
                        Some(e) => {
                            format!("failed with: {}", e)
                        }
                    };
                    println!(
                        " - {} took {} ms and {}",
                        phase.phase,
                        phase
                            .elapsed
                            .expect("Unable to convert elapsed time")
                            .as_nanos()
                            / 1000000,
                        result,
                    );
                }
            }
        }
        SupportCommands::SupportTunnel { enable } => {
            // This is independent of the implementation, which will be plumbed through once we
            // have an implementation for https://github.com/Holo-Host/holo-host-private/issues/14.
            match enable {
                true => {
                    println!("Support Tunnel not yet implemented")
                }
                false => {
                    println!("Support Tunnel already disabled")
                }
            }
        }
    }
    Ok(())
}
