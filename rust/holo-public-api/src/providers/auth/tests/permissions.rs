mod tests {
    use crate::providers::auth::permissions;
    use crate::providers::jwt::AccessTokenClaims;
    use db_utils::schemas::user_permissions;

    #[test]
    fn should_succeed_if_user_has_correct_permission_action() {
        let all_actions = vec![
            user_permissions::PermissionAction::Read,
            user_permissions::PermissionAction::Update,
            user_permissions::PermissionAction::Delete,
            user_permissions::PermissionAction::All,
        ];

        for action in all_actions {
            assert!(permissions::verify_permission(
                AccessTokenClaims {
                    sub: "user_id".to_string(),
                    permissions: vec![user_permissions::UserPermission {
                        resource: "resource".to_string(),
                        action: action.clone(),
                        owner: "user_id".to_string(),
                    }],
                    exp: 0, // not used
                },
                user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action: action.clone(),
                    owner: "user_id".to_string(),
                },
            ));

            assert!(permissions::verify_permission(
                AccessTokenClaims {
                    sub: "user_id".to_string(),
                    permissions: vec![user_permissions::UserPermission {
                        resource: "resource".to_string(),
                        action: user_permissions::PermissionAction::All,
                        owner: "user_id".to_string(),
                    }],
                    exp: 0, // not used
                },
                user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action,
                    owner: "user_id".to_string(),
                },
            ));
        }
    }

    #[test]
    fn should_succeed_if_user_has_ownership() {
        assert!(permissions::verify_permission(
            AccessTokenClaims {
                sub: "user_id".to_string(),
                permissions: vec![user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action: user_permissions::PermissionAction::Read,
                    owner: "self".to_string(),
                }],
                exp: 0, // not used
            },
            user_permissions::UserPermission {
                resource: "resource".to_string(),
                action: user_permissions::PermissionAction::Read,
                owner: "user_id".to_string(),
            },
        ));

        assert!(permissions::verify_permission(
            AccessTokenClaims {
                sub: "user_id".to_string(),
                permissions: vec![user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action: user_permissions::PermissionAction::Read,
                    owner: "user_id2".to_string(),
                }],
                exp: 0, // not used
            },
            user_permissions::UserPermission {
                resource: "resource".to_string(),
                action: user_permissions::PermissionAction::Read,
                owner: "user_id2".to_string(),
            },
        ));

        assert!(permissions::verify_permission(
            AccessTokenClaims {
                sub: "user_id".to_string(),
                permissions: vec![user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action: user_permissions::PermissionAction::Read,
                    owner: "all".to_string(),
                }],
                exp: 0, // not used
            },
            user_permissions::UserPermission {
                resource: "resource".to_string(),
                action: user_permissions::PermissionAction::Read,
                owner: "user_id2".to_string(),
            },
        ));
    }

    #[test]
    fn should_succeed_if_user_has_access_to_resource() {
        assert!(permissions::verify_permission(
            AccessTokenClaims {
                sub: "user_id".to_string(),
                permissions: vec![user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action: user_permissions::PermissionAction::Read,
                    owner: "user_id".to_string(),
                }],
                exp: 0, // not used
            },
            user_permissions::UserPermission {
                resource: "resource".to_string(),
                action: user_permissions::PermissionAction::Read,
                owner: "user_id".to_string(),
            },
        ));

        assert!(permissions::verify_permission(
            AccessTokenClaims {
                sub: "user_id".to_string(),
                permissions: vec![user_permissions::UserPermission {
                    resource: "all".to_string(),
                    action: user_permissions::PermissionAction::Read,
                    owner: "user_id".to_string(),
                }],
                exp: 0, // not used
            },
            user_permissions::UserPermission {
                resource: "resource".to_string(),
                action: user_permissions::PermissionAction::Read,
                owner: "user_id".to_string(),
            },
        ));
    }

    #[test]
    fn should_fail_if_user_has_not_access_to_resource() {
        assert!(!permissions::verify_permission(
            AccessTokenClaims {
                sub: "user_id".to_string(),
                permissions: vec![user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action: user_permissions::PermissionAction::Read,
                    owner: "user_id".to_string(),
                }],
                exp: 0, // not used
            },
            user_permissions::UserPermission {
                resource: "other_resource".to_string(),
                action: user_permissions::PermissionAction::Read,
                owner: "user_id".to_string(),
            },
        ));
    }

    #[test]
    fn should_fail_if_user_has_not_access_to_action() {
        assert!(!permissions::verify_permission(
            AccessTokenClaims {
                sub: "user_id".to_string(),
                permissions: vec![user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action: user_permissions::PermissionAction::Read,
                    owner: "user_id".to_string(),
                }],
                exp: 0, // not used
            },
            user_permissions::UserPermission {
                resource: "resource".to_string(),
                action: user_permissions::PermissionAction::Update,
                owner: "user_id".to_string(),
            },
        ));
    }

    #[test]
    fn should_fail_if_user_is_not_owner() {
        assert!(!permissions::verify_permission(
            AccessTokenClaims {
                sub: "user_id".to_string(),
                permissions: vec![user_permissions::UserPermission {
                    resource: "resource".to_string(),
                    action: user_permissions::PermissionAction::Read,
                    owner: "user_id".to_string(),
                }],
                exp: 0, // not used
            },
            user_permissions::UserPermission {
                resource: "resource".to_string(),
                action: user_permissions::PermissionAction::Read,
                owner: "user_id2".to_string(),
            },
        ));
    }
}
