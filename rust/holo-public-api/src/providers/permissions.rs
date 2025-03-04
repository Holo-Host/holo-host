use std::collections::HashMap;

use actix_web::{HttpMessage, HttpRequest};

use super::{database::schemas::user::User, jwt::AccessTokenClaims};

// workload permissions
pub const WORKLOADS_CREATE: &str = "workloads.create";
pub const WORKLOADS_READ: &str = "workloads.read";
pub const WORKLOADS_READ_ALL: &str = "workloads.read.all";
pub const WORKLOADS_DELETE: &str = "workloads.delete";
pub const WORKLOADS_DELETE_ALL: &str = "workloads.delete.all";
pub const WORKLOADS_UPDATE: &str = "workloads.update";
pub const WORKLOADS_UPDATE_ALL: &str = "workloads.update.all";

// host permissions
pub const HOST_CREATE: &str = "host.create";
pub const HOST_READ: &str = "host.read";
pub const HOST_READ_ALL: &str = "host.read.all";
pub const HOST_DELETE: &str = "host.delete";
pub const HOST_DELETE_ALL: &str = "host.delete.all";
pub const HOST_UPDATE: &str = "host.update";
pub const HOST_UPDATE_ALL: &str = "host.update.all";

// user permissions
pub const USER_CREATE: &str = "user.create";
pub const USER_READ: &str = "user.read";
pub const USER_READ_ALL: &str = "user.read.all";
pub const USER_DELETE: &str = "user.delete";
pub const USER_DELETE_ALL: &str = "user.delete.all";
pub const USER_UPDATE: &str = "user.update";
pub const USER_UPDATE_ALL: &str = "user.update.all";

pub fn get_claims_from_req(req: HttpRequest) -> Option<AccessTokenClaims> {
    let extensions = req.extensions();
    let claims = match extensions.get::<AccessTokenClaims>() {
        Some(claims) => claims,
        None => return None,
    };
    Some(claims.clone())
}

pub fn verify_user_has_permission(claims: AccessTokenClaims, required_permissions: Vec<String>) -> Option<String> {
    for required_permission in required_permissions {
        if claims.permissions.contains(&required_permission) {
            return Some(required_permission);
        }
    }
    None
}

pub fn get_roles() -> HashMap<String, Vec<String>> {
    vec![
        ("admin".to_string(), vec![
            WORKLOADS_CREATE.to_string(),
            WORKLOADS_READ_ALL.to_string(),
            WORKLOADS_DELETE_ALL.to_string(),
            WORKLOADS_UPDATE_ALL.to_string(),

            HOST_CREATE.to_string(),
            HOST_READ_ALL.to_string(),
            HOST_DELETE_ALL.to_string(),
            HOST_UPDATE_ALL.to_string(),

            USER_CREATE.to_string(),
            USER_READ_ALL.to_string(),
            USER_DELETE_ALL.to_string(),
            USER_UPDATE_ALL.to_string(),
        ]),
        ("developer".to_string(), vec![
            WORKLOADS_READ.to_string(),
            WORKLOADS_CREATE.to_string(),
            WORKLOADS_DELETE.to_string(),
            WORKLOADS_UPDATE.to_string(),
            USER_READ.to_string(),
        ]),
        ("hoster".to_string(), vec![
            HOST_READ.to_string(),
            USER_READ.to_string(),
            HOST_CREATE.to_string(),
            HOST_DELETE.to_string(),
            HOST_UPDATE.to_string(),
        ]),
    ]
    .into_iter()
    .collect()
}

pub fn get_permissions_from_roles(roles: Vec<String>) -> Vec<String> {
    let roles_map = get_roles();
    roles.iter().map(|role| roles_map[role].clone()).flatten().collect()
}

pub fn get_user_permissions(user: User) -> Vec<String> {
    let mut permissions = user.permissions.clone();
    permissions.extend(get_permissions_from_roles(user.roles.clone()));
    permissions
}

#[cfg(test)]
mod tests {
    use crate::providers::database::schemas::shared::meta::Meta;

    use super::*;
    
    #[test]
    fn should_return_permissions_for_admin_role() {
        let user = User {
            oid: Some(bson::oid::ObjectId::new()),
            meta: Meta {
                created_at: bson::DateTime::now(),
                updated_at: bson::DateTime::now(),
                deleted_at: None,
                is_deleted: false,
            },
            refresh_token_version: 0,
            permissions: vec![],
            roles: vec!["admin".to_string()],
        };
        let permissions = get_user_permissions(user);
        let admin_permissions = get_roles()["admin"].clone();
        assert_eq!(permissions.len(), admin_permissions.len());
        for permission in admin_permissions.iter() {
            assert!(permissions.contains(permission));
        }
    }

    #[test]
    fn should_return_permissions_for_developer_role() {
        let user = User {
            oid: Some(bson::oid::ObjectId::new()),
            meta: Meta {
                created_at: bson::DateTime::now(),
                updated_at: bson::DateTime::now(),
                deleted_at: None,
                is_deleted: false,
            },
            refresh_token_version: 0,
            permissions: vec![],
            roles: vec!["developer".to_string()],
        };
        let permissions = get_user_permissions(user);
        let developer_permissions = get_roles()["developer"].clone();
        assert_eq!(permissions.len(), developer_permissions.len());
        for permission in developer_permissions.iter() {
            assert!(permissions.contains(permission));
        }
    }
}
