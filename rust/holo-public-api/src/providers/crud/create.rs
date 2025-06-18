use bson::oid::ObjectId;
use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Create a new document in the specified MongoDB collection.
/// Returns the ObjectId of the created document.
pub async fn create<T>(
    db: mongodb::Client,
    collection_name: String,
    item: T,
) -> Result<ObjectId, anyhow::Error>
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
    let result = match collection.insert_one_into(item).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to insert document: {}", e);
            return Err(anyhow::anyhow!("Failed to insert document"));
        }
    };
    Ok(result)
}
