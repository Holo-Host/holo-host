use bson::DateTime;
use serde::{Deserialize, Serialize};

/// Common metadata fields for database documents
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    /// Flag indicating if the document has been marked as deleted
    pub is_deleted: bool,
    /// Timestamp when the document was deleted
    pub deleted_at: Option<bson::DateTime>,
    /// Timestamp of the last update
    pub updated_at: Option<bson::DateTime>,
    /// Timestamp when the document was created
    pub created_at: Option<bson::DateTime>,
}

impl Default for Metadata {
    fn default() -> Self {
        let dt = DateTime::now();
        Metadata {
            is_deleted: false,
            deleted_at: None,
            updated_at: Some(dt),
            created_at: Some(dt),
        }
    }
}
