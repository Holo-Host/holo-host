#[cfg(test)]
mod test {
    use crate::dns_cache;
    use log::debug;
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::TempDir;

    fn test_init() {
        env_logger::init();
        std::env::set_var("DNS_CACHE_UPDATE_TIME", "2");
        std::env::set_var("DNS_SERVER_PORT", "8053");
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
}
