use db_utils::mongodb::{
    api::MongoDbAPI,
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug};

/// Hard delete a document by its ID in the specified MongoDB collection.
/// Returns an empty result if successful.
pub async fn delete_hard<T>(
    db: mongodb::Client,
    collection_name: String,
    id: String,
) -> Result<(), anyhow::Error>
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
    match collection.delete_one_from(bson::doc! { "_id": id }).await {
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::error!("Failed to delete document: {}", e);
            Err(anyhow::anyhow!("Failed to delete document"))
        }
    }
}
