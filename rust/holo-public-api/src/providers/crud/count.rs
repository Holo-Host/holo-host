use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug)]
pub struct Count {
    pub count: i32,
}

/// Get all documents from the specified MongoDB collection.
/// Returns a vector of documents.
/// If the documents are marked as deleted, they will not be returned.
/// Supports filtering, sorting, limiting, and skipping.
/// If no filter is provided, all documents will be returned.
/// If no sort is provided, the documents will be sorted by `_id` in ascending order.
/// If no limit is provided, a default limit of 100 will be used.
/// If no skip is provided, a default skip of 0 will be used.
pub async fn count<T>(
    db: mongodb::Client,
    collection_name: String,
    filter: Option<bson::Document>,
) -> Result<i32, anyhow::Error>
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
    let result = match collection
        .aggregate::<Count>(vec![
            bson::doc! { "$match": { "metadata.is_deleted": false } },
            bson::doc! { "$match": filter.unwrap_or_default() },
            bson::doc! { "$count": "count" },
        ])
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to get all documents: {}", e);
            return Err(anyhow::anyhow!("Failed to get all documents"));
        }
    };
    if result.is_empty() {
        return Ok(0);
    }
    let count = result[0].count;
    Ok(count)
}
