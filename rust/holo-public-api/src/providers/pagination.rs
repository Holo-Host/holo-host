use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Pagination<T> {
    pub items: Vec<T>, // list of items
    pub total: i32, // total documents count from db
    pub page: i32, // current page
    pub limit: i32, // page limit
}

#[derive(Debug, Serialize, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PaginationQuery {
    pub page: i32,
    pub limit: i32,
}