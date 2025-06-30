#[cfg(test)]
mod test {
    use crate::dns_cache;
    use crate::dns_service::resolver_handler;
    use dns_server::{DnsClass, DnsName, DnsQuestion, DnsType};
    use log::debug;
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};
    use std::sync::Once;
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::TempDir;

    const TEST_SERVER_PORT: u16 = 53000;

    static INIT: Once = Once::new();

    fn test_init() {
        INIT.call_once(|| {
            env_logger::init();
            std::env::set_var("DNS_CACHE_UPDATE_TIME", "2");
            std::env::set_var("DNS_SERVER_PORT", format!("{}", TEST_SERVER_PORT));
        })
    }

    /// This test spins up the cache-loop thread and ensures that it updates the cache data as the
    /// backend data source is updated independently by the test.
    #[tokio::test(flavor = "multi_thread")]
    async fn file_cache_test() {
        test_init();

        let mut cache_source = HashMap::<String, dns_cache::DnsCacheItem>::new();

        let tmp = TempDir::new().expect("Can't create tempdir");
        let json_file: String = format!("{}/test.json", tmp.path().to_string_lossy());

        let mut file = File::create(&json_file).unwrap();
        let data = serde_json::to_string(&cache_source).unwrap();
        debug!("Initial data: {}", &data);
        let _ = file.write(data.as_bytes());

        let ret = dns_cache::start_cache(dns_cache::DnsCacheSource::JsonFile(
            dns_cache::JsonFileSourceParms {
                json_file: json_file.clone(),
            },
        ))
        .await;
        debug!("ret: {:?}", ret);

        // Cache should start empty.
        {
            let cache = crate::dns_cache::DNS_CACHE.read().unwrap();
            debug!("cache should be empty: {:?}", cache);
            assert!(cache.is_empty());
        }

        cache_source.insert(
            "x-service-test.dna.holo.host".to_string(),
            dns_cache::DnsCacheItem {
                a: vec!["127.0.0.1".to_string()],
                aaaa: vec!["::1".to_string()],
                cname: vec!["x-cname-test.dna.holo.host".to_string()],
                ns: vec!["x-service-test.dna.holo.host".to_string()],
            },
        );
        file.seek(SeekFrom::Start(0)).unwrap();
        let data = serde_json::to_string(&cache_source).unwrap();
        debug!("Data written: {} to {}", data, json_file);
        let ret = file.write(data.as_bytes());
        debug!("Data ret: {:?}", ret);

        // Wait long enough for the cache thread to pick up our changes
        sleep(Duration::from_secs(3));
        // Cache should contain the contents of the file.
        {
            let cache = crate::dns_cache::DNS_CACHE.read().unwrap();
            debug!("cache should be populated: {:?}", cache);
            assert!(cache.contains_key("x-service-test.dna.holo.host"));
        }
    }

    /// This checks that we can resolve names to addresses from the cache.
    #[tokio::test(flavor = "multi_thread")]
    async fn lookup_tests() {
        test_init();

        // Populate the cache with some test records
        let mut cache_source = HashMap::<String, dns_cache::DnsCacheItem>::new();
        cache_source.insert(
            "x-service-test.dna.holo.host".to_string(),
            dns_cache::DnsCacheItem {
                a: vec!["127.0.0.1".to_string()],
                aaaa: vec!["::1".to_string()],
                cname: vec!["x-cname-test.dna.holo.host".to_string()],
                ns: vec!["x-ns-test.dna.holo.host".to_string()],
            },
        );

        let tmp = TempDir::new().expect("Can't create tempdir");
        let json_file: String = format!("{}/test.json", tmp.path().to_string_lossy());

        let mut file = File::create(&json_file).unwrap();
        let data = serde_json::to_string(&cache_source).unwrap();
        debug!("Data written: {} to {}", data, json_file);
        let ret = file.write(data.as_bytes());
        debug!("Data ret: {:?}", ret);

        let ret = dns_cache::start_cache(dns_cache::DnsCacheSource::JsonFile(
            dns_cache::JsonFileSourceParms {
                json_file: json_file.clone(),
            },
        ))
        .await;
        debug!("ret: {:?}", ret);

        // Wait long enough for the cache thread to pick up our changes
        sleep(Duration::from_secs(3));
        debug!("Do stuff here");

        let queries: Vec<DnsQuestion> = vec![
            // This query does an A record sanity check
            DnsQuestion {
                name: DnsName::new("x-service-test.dna.holo.host.").unwrap(),
                class: DnsClass::Internet,
                typ: DnsType::A,
            },
            // This query does an AAAA record sanity check
            DnsQuestion {
                name: DnsName::new("x-service-test.dna.holo.host.").unwrap(),
                class: DnsClass::Internet,
                typ: DnsType::AAAA,
            },
            // This query makes sure that we're case-insensitive.
            DnsQuestion {
                name: DnsName::new("x-Service-Test.DNA.Holo.Host.").unwrap(),
                class: DnsClass::Internet,
                typ: DnsType::A,
            },
            // Sanity check CNAMEs
            DnsQuestion {
                name: DnsName::new("x-service-test.dna.holo.host.").unwrap(),
                class: DnsClass::Internet,
                typ: DnsType::CNAME,
            },
            // Sanity check NS records
            DnsQuestion {
                name: DnsName::new("x-service-test.dna.holo.host.").unwrap(),
                class: DnsClass::Internet,
                typ: DnsType::NS,
            },
        ];

        for query in queries {
            let result = resolver_handler(&query);
            // The records above are all designed to return a single entry at the moment.
            debug!("{} -> {:?}", query.name, result);
            assert!(result.len() == 1);
        }
    }
}
