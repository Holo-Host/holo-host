use bson::oid::ObjectId;
use db_utils::schemas::user_permissions::{PermissionAction, UserPermission};

use crate::providers::jwt::AccessTokenClaims;

/// This function is used to verify if the user has any of the required permissions
///
/// Parameters:
/// - `user_id`: The user id
/// - `user_permissions`: The permissions the user has
/// - `required_permissions`: The permissions required (cannot have 'self' as the owner)
///
/// Returns:
/// - `true` if the user has any of the required permissions
/// - `false` if the user does not have any of the required permissions
pub fn verify_any_permissions(
    claims: AccessTokenClaims,
    required_permissions: Vec<UserPermission>,
) -> bool {
    let user_id = claims.sub;
    let user_permissions = claims.permissions;
    for required_permission in required_permissions {
        for user_permission in user_permissions.clone() {
            if required_permission.resource != user_permission.resource
                && user_permission.resource != "all"
            {
                continue;
            }
            if required_permission.action != user_permission.action
                && user_permission.action != PermissionAction::All
            {
                continue;
            }
            if user_permission.owner == "all" {
                return true;
            }
            if user_id.clone() == required_permission.owner && user_permission.owner == "self" {
                return true;
            }
            if required_permission.owner != user_permission.owner {
                continue;
            }
            if required_permission.owner == "all" && user_permission.owner != "all" {
                continue;
            }
            return true;
        }
    }
    false
}

/// This function is used to verify if the user has all the required permissions
///
/// Parameters:
/// - `user_id`: The user id
/// - `user_permissions`: The permissions the user has
/// - `required_permissions`: The permissions required (cannot have 'self' as the owner)
///
/// Returns:
/// - `true` if the user has all the required permissions
/// - `false` if the user does not have all the required permissions
pub fn verify_all_permissions(
    claims: AccessTokenClaims,
    required_permissions: Vec<UserPermission>,
) -> bool {
    for required_permission in required_permissions {
        if !verify_any_permissions(claims.clone(), vec![required_permission]) {
            return false;
        }
    }
    true
}

/// This function is used to get all the owners of a resource
/// It filters the permissions by resource and action
/// It returns a vector of ObjectId
pub fn get_all_accessible_owners_from_permissions(
    user_permissions: Vec<UserPermission>,
    resource: String,
    action: PermissionAction,
    user_id: String,
) -> Vec<ObjectId> {
    let mut owner: Vec<ObjectId> = user_permissions
        .into_iter()
        .filter_map(|claim| {
            if claim.resource != resource && claim.resource != "all" {
                return None;
            }
            if claim.action != action && claim.action != PermissionAction::All {
                return None;
            }

            match ObjectId::parse_str(claim.owner) {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        })
        .collect();
    let user_oid = ObjectId::parse_str(user_id).expect("failed to parse user id");
    owner.push(user_oid);
    owner
}
