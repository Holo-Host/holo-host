use dns_server::{DnsClass, DnsQuestion, DnsRecord, DnsType};
use log::{debug, info};
use permit::Permit;

pub fn start_service(port: u16, permit: &Permit) {
    info!("Listening on port {}", port);
    dns_server::Builder::new_port(port)
        .unwrap()
        .with_permit(permit.clone())
        .serve(&resolver_handler)
        .unwrap();
}

/// This is the callback from the DNS server that handles the incoming requests and formluates
/// responses.
fn resolver_handler(question: &DnsQuestion) -> Vec<DnsRecord> {
    debug!("Received DNS query: {:?}", question);
    // No ChaosNet today. Internet records only.
    if question.class != DnsClass::Internet {
        debug!("Query for unknown class: {:?}", question);
        return vec![];
    }

    debug!("Requesting read lock.");
    let cache = crate::dns_cache::DNS_CACHE.read().unwrap();
    debug!("Got read lock");
    // Return the desired type of record, as applicable. Otherwise an empty record set.
    match question.typ {
        DnsType::A => {
            let mut ret = vec![];
            debug!("Confirming A query for {}", &question.name.to_string());
            if cache.contains_key(&question.name.to_string()) {
                // Add names to ret. TODO: We should randomise this list.
                for rec in cache[&question.name.to_string()].a.iter() {
                    ret.push(DnsRecord::A(question.name.clone(), rec.parse().unwrap()));
                }
            }
            debug!("ret: {:?}", ret);
            ret
        }
        DnsType::AAAA => {
            let mut ret = vec![];
            debug!("Confirming AAAA query for {}", &question.name.to_string());
            if cache.contains_key(&question.name.to_string()) {
                // Add names to ret
                for rec in cache[&question.name.to_string()].aaaa.iter() {
                    ret.push(DnsRecord::AAAA(question.name.clone(), rec.parse().unwrap()));
                }
            }
            debug!("ret: {:?}", ret);
            ret
        }
        _ => {
            debug!("Query for unknown type: {:?}", question);
            vec![]
        }
    }
}
