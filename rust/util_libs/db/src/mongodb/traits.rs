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

// todo: we should generate mongodb validation rules based on the schema
pub trait Validation {
    fn build_validation(&self) -> Result<bson::Document>;
}
