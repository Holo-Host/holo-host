use bson::oid::ObjectId;
use db_utils::mongodb::traits::{IntoIndexes, MutMetadata, WithMongoDbId};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::providers::crud;

/// Update a document by its ID in the specified MongoDB collection.
/// Returns the result of the update operation.
/// If the document is marked as deleted, it will not be updated.
pub async fn upsert<T>(
    db: mongodb::Client,
    collection_name: String,
    filter: bson::Document,
    data: T,
) -> Result<ObjectId, anyhow::Error>
where
    T: Serialize
        + for<'de> Deserialize<'de>
        + Unpin
        + Send
        + Sync
        + Default
        + Clone
        + Debug
        + IntoIndexes
        + MutMetadata
        + WithMongoDbId,
{
    let existing = crud::find_one::<T>(db.clone(), collection_name.clone(), filter.clone()).await?;
    let oid = match existing {
        Some(existing) => {
            let oid = existing.get_id();
            crud::update::<T>(
                db.clone(),
                collection_name.clone(),
                oid.to_hex(),
                bson::ser::to_document(&data.clone())?,
            )
            .await?;
            oid
        }
        None => crud::create::<T>(db.clone(), collection_name.clone(), data.clone()).await?,
    };
    Ok(oid)
}
