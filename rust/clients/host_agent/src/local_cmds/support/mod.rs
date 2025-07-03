pub mod errors;
pub mod types;

use errors::{SupportError, SupportResult};
use types::SupportCommands;

use netdiag::*;

pub fn support_command(command: &SupportCommands) -> SupportResult<()> {
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
            let mut any_failed = false;

            for stat in stats.iter() {
                match &stat.result {
                    Ok(_) => {
                        println!("Request succeeded");
                    }
                    Err(e) => {
                        println!("Request failed with: {}", e);
                        any_failed = true;
                    }
                }
                for phase in &stat.stats {
                    let result = match &phase.error {
                        None => "succeeded".to_string(),
                        Some(e) => {
                            any_failed = true;
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

            if any_failed {
                return Err(SupportError::diagnostic_failed(
                    "network connectivity test",
                    "One or more network tests failed",
                ));
            }
        }
        SupportCommands::SupportTunnel { enable } => {
            // This is independent of the implementation, which will be plumbed through once we
            // have an implementation for https://github.com/Holo-Host/holo-host-private/issues/14.
            match enable {
                true => {
                    return Err(SupportError::diagnostic_failed(
                        "support tunnel",
                        "Support tunnel functionality not yet implemented",
                    ));
                }
                false => {
                    println!("Support Tunnel already disabled");
                }
            }
        }
    }
    Ok(())
}
