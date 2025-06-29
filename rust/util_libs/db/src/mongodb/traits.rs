use crate::schemas::metadata::Metadata;
use anyhow::Result;
use bson::Document;
use mongodb::options::IndexOptions;

/// Trait for defining MongoDB indices for a collection.
///
/// Implementors of this trait can define the indices that should be created
/// for their corresponding MongoDB collection.
pub trait IntoIndexes {
    /// Converts the implementation into a vector of index definitions.
    ///
    /// # Returns
    ///
    /// A vector of tuples containing the index specification document and optional index options
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>>;
}

pub trait MutMetadata {
    fn mut_metadata(&mut self) -> &mut Metadata;
}

#[macro_export]
macro_rules! derive_with_metadata {
    ($type:ty) => {
        impl $crate::mongodb::traits::MutMetadata for $type {
            fn mut_metadata(&mut self) -> &mut Metadata {
                &mut self.metadata
            }
        }
    };
}

// todo: we should generate mongodb validation rules based on the schema
pub trait Validation {
    fn build_validation(&self) -> Result<bson::Document>;
}

pub trait WithMongoDbId {
    fn _id(&self) -> Option<bson::oid::ObjectId>;

    fn get_id(&self) -> bson::oid::ObjectId {
        self._id().expect("id not set")
    }
    fn get_id_string(&self) -> String {
        self.get_id().to_hex()
    }
}

#[macro_export]
macro_rules! derive_with_mongo_id {
    ($type:ty) => {
        impl $crate::mongodb::traits::WithMongoDbId for $type {
            fn _id(&self) -> Option<ObjectId> {
                self._id
            }
        }
    };
}
