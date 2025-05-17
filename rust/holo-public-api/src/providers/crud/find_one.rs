use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Get a single document using a filter from the specified MongoDB collection.
/// Returns the document if found, otherwise returns None.
/// If the document is marked as deleted, it will not be returned.
pub async fn find_one<T>(
    db: mongodb::Client,
    collection_name: String,
    filter: bson::Document,
) -> Result<Option<T>, anyhow::Error>
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
    let mut filter = filter;
    filter.insert("metadata.is_deleted", false);
    let result = match collection.get_one_from(filter).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to get document by ID: {}", e);
            return Err(anyhow::anyhow!("Failed to get document by ID"));
        }
    };
    Ok(result)
}
