use serde::{Deserialize, Serialize};
use utoipa::{schema, ToSchema};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct PaginationResponse<T> {
    /// The current page number
    /// The page number starts from 1
    #[schema(example = 1)]
    pub page: i32,

    /// The number of items per page
    /// This is the maximum number of items that can be returned in a single page
    #[schema(example = 10)]
    pub limit: i32,

    /// The total number of items
    /// This is the total number of items in the database
    pub total: i32,

    /// list of items in the current page
    pub items: Vec<T>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct PaginationRequest {
    /// The page number
    /// The page number starts from 1
    #[schema(example = 1)]
    pub page: i32,

    /// The number of items per page
    /// This is the maximum number of items that can be returned in a single page
    #[schema(example = 10)]
    pub limit: i32,
}
