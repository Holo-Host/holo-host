use db_utils::schemas::{
    user::UserRole,
    user_permissions::{PermissionAction, UserPermission},
};

pub fn get_role_permissions(role: UserRole) -> Vec<UserPermission> {
    match role {
        UserRole::Admin => vec![UserPermission {
            resource: String::from("all"),
            action: PermissionAction::All,
            owner: String::from("all"),
        }],
        UserRole::User => vec![UserPermission {
            resource: String::from("all"),
            action: PermissionAction::All,
            owner: String::from("self"),
        }],
        UserRole::Support => vec![
            UserPermission {
                resource: String::from("all"),
                action: PermissionAction::All,
                owner: String::from("self"),
            },
            UserPermission {
                resource: String::from("all"),
                action: PermissionAction::Read,
                owner: String::from("all"),
            },
        ],
    }
}

pub fn combine_role_and_permissions(
    roles: Vec<UserRole>,
    permissions: Vec<UserPermission>,
) -> Vec<UserPermission> {
    let mut role_permissions: Vec<UserPermission> = vec![];
    for role in roles.clone() {
        for p in get_role_permissions(role) {
            role_permissions.push(p);
        }
    }
    for p in permissions {
        role_permissions.push(p);
    }
    role_permissions
}
