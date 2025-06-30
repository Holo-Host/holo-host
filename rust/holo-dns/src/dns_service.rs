use dns_server::{DnsClass, DnsName, DnsQuestion, DnsRecord, DnsType};
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
pub(crate) fn resolver_handler(question: &DnsQuestion) -> Vec<DnsRecord> {
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
            if cache.contains_key(&question.name.to_string().to_lowercase()) {
                // Add names to ret. TODO: We should randomise this list.
                for rec in cache[&question.name.to_string().to_lowercase()].a.iter() {
                    ret.push(DnsRecord::A(question.name.clone(), rec.parse().unwrap()));
                }
            }
            debug!("ret: {:?}", ret);
            ret
        }
        DnsType::AAAA => {
            let mut ret = vec![];
            debug!("Confirming AAAA query for {}", &question.name.to_string());
            if cache.contains_key(&question.name.to_string().to_lowercase()) {
                // Add names to ret
                for rec in cache[&question.name.to_string().to_lowercase()].aaaa.iter() {
                    ret.push(DnsRecord::AAAA(question.name.clone(), rec.parse().unwrap()));
                }
            }
            debug!("ret: {:?}", ret);
            ret
        }
        DnsType::CNAME => {
            let mut ret = vec![];
            debug!("Confirming CNAME record lookup for {:?}", question);
            if cache.contains_key(&question.name.to_string().to_lowercase()) {
                for rec in cache[&question.name.to_string().to_lowercase()]
                    .cname
                    .iter()
                {
                    ret.push(DnsRecord::CNAME(
                        question.name.clone(),
                        DnsName::new(rec).unwrap(),
                    ));
                }
            }
            debug!("ret: {:?}", ret);
            ret
        }
        DnsType::NS => {
            let mut ret = vec![];
            debug!("Confirming NS record lookup for {:?}", question);
            if cache.contains_key(&question.name.to_string().to_lowercase()) {
                for rec in cache[&question.name.to_string().to_lowercase()].ns.iter() {
                    ret.push(DnsRecord::NS(
                        question.name.clone(),
                        DnsName::new(rec).unwrap(),
                    ));
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
