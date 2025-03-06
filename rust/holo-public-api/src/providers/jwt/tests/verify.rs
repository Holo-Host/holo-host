#[cfg(test)]
mod tests {
    use crate::providers::jwt::{verify_jwt, AccessTokenClaims};

    const TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiI2N2I2NmI3MzM0NjFiYWRhM2EyZTgxNTMiLCJleHAiOjAsInBlcm1pc3Npb25zIjpbXX0.5jYCYlomei0bTBy-bvAHg3vkrCzrRPLaeTn8MTyFFUY";
    const SECRET: &str = "jwt_secret";

    #[test]
    fn should_succeed_to_verify_access_token() {
        let result = verify_jwt::<AccessTokenClaims>(TOKEN, SECRET).unwrap();
        assert_eq!(result.sub, "67b66b733461bada3a2e8153".to_string());
        assert_eq!(result.exp, 0);
        assert_eq!(result.permissions.len(), 0);
    }

    #[test]
    fn should_fail_to_verify_access_token() {
        let invalid_token = "invalid_token";
        let result = verify_jwt::<AccessTokenClaims>(invalid_token, SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn should_fail_to_verify_access_token_with_invalid_secret() {
        let invalid_secret = "invalid_secret";
        let result = verify_jwt::<AccessTokenClaims>(TOKEN, invalid_secret);
        assert!(result.is_err());
    }
}
