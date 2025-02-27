#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;
use crate::db::mongodb::{MongoCollection, MongoDbAPI};
use crate::db::schemas::{self, Capacity, Metadata};
use bson::{self, doc, oid, DateTime};
use dotenv::dotenv;

#[cfg(not(target_arch = "aarch64"))]
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_indexing_and_api() -> Result<()> {
        dotenv().ok();
        env_logger::init();

        let mongod = mongo_runner::MongodRunner::run().unwrap();
        let client = mongod.client().unwrap();

        let database_name = "holo-hosting-test";
        let collection_name = "host";
        let mut host_api =
            MongoCollection::<schemas::Host>::new(&client, database_name, collection_name).await?;

        // set index
        host_api.apply_indexing().await?;

        fn get_mock_host() -> schemas::Host {
            schemas::Host {
                _id: Some(oid::ObjectId::new()),
                metadata: Metadata {
                    is_deleted: false,
                    created_at: Some(DateTime::now()),
                    updated_at: Some(DateTime::now()),
                    deleted_at: None,
                },
                device_id: "placeholder_pubkey_host".to_string(),
                ip_address: "127.0.0.1".to_string(),
                remaining_capacity: Capacity {
                    memory: 16,
                    disk: 200,
                    cores: 16,
                },
                avg_uptime: 0.95,
                avg_network_speed: 500,
                avg_latency: 10,
                assigned_workloads: vec![oid::ObjectId::new()],
                assigned_hoster: oid::ObjectId::new(),
            }
        }

        // insert a document
        let host_0 = get_mock_host();
        let r = host_api.insert_one_into(host_0.clone()).await?;
        println!("result : {:?}", r);

        // get one (the same) document
        println!("host_0._id.unwrap() : {:?}", host_0._id.unwrap());
        let filter_one = doc! { "_id":  host_0._id.unwrap() };
        let fetched_host = host_api.get_one_from(filter_one.clone()).await?;
        let mongo_db_host = fetched_host.expect("Failed to fetch host");
        assert_eq!(mongo_db_host._id, host_0._id);

        // insert many documents
        let host_1 = get_mock_host();
        let host_2 = get_mock_host();
        let host_3 = get_mock_host();
        host_api.insert_one_into(host_1.clone()).await?;
        host_api.insert_one_into(host_2.clone()).await?;
        host_api.insert_one_into(host_3.clone()).await?;

        // get many docs
        let ids = vec![
            host_1._id.unwrap(),
            host_2._id.unwrap(),
            host_3._id.unwrap(),
        ];
        let filter_many = doc! {
            "_id": { "$in": ids.clone() }
        };
        let fetched_hosts = host_api.get_many_from(filter_many.clone()).await?;

        assert_eq!(fetched_hosts.len(), 3);
        let updated_ids: Vec<oid::ObjectId> = fetched_hosts
            .into_iter()
            .map(|h| h._id.unwrap_or_default())
            .collect();
        assert!(updated_ids.contains(&ids[0]));
        assert!(updated_ids.contains(&ids[1]));
        assert!(updated_ids.contains(&ids[2]));

        // Delete collection and all documents therein.
        let _ = host_api.inner.drop();

        Ok(())
    }
}
