use bson::oid::ObjectId;
use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug)]
pub struct Owner {
    pub owner: ObjectId,
}

pub async fn get_owner<T>(
    db: mongodb::Client,
    collection_name: String,
    id: String,
) -> Result<Option<String>, anyhow::Error>
where
    T: Serialize
        + for<'de> Deserialize<'de>
        + Unpin
        + Send
        + Sync
        + Default
        + Debug
        + IntoIndexes
        + MutMetadata,
{
    let collection = match MongoCollection::<T>::new(&db, "holo", &collection_name).await {
        Ok(collection) => collection,
        Err(e) => {
            tracing::error!("Failed to create MongoDB collection: {}", e);
            return Err(anyhow::anyhow!("Failed to create MongoDB collection"));
        }
    };
    let oid = match ObjectId::parse_str(id) {
        Ok(oid) => oid,
        Err(_) => {
            return Err(anyhow::anyhow!("Failed to parse mongodb id"));
        }
    };
    let result = match collection
        .aggregate::<Owner>(vec![
            bson::doc! { "$match": { "_id": oid, "metadata.is_deleted": false } },
            bson::doc! { "$project": { "owner": 1 } },
        ])
        .await
    {
        Ok(result) => result,
        Err(_) => {
            return Err(anyhow::anyhow!("Failed to get owner"));
        }
    };
    if result.is_empty() {
        return Ok(None);
    }
    Ok(Some(result[0].owner.to_hex()))
}
