#[cfg(test)]
mod tests {
    use crate::providers::jwt::{sign_jwt, AccessTokenClaims};

    const USER_ID: &str = "67b66b733461bada3a2e8153";
    const TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiI2N2I2NmI3MzM0NjFiYWRhM2EyZTgxNTMiLCJleHAiOjAsInBlcm1pc3Npb25zIjpbXX0.5jYCYlomei0bTBy-bvAHg3vkrCzrRPLaeTn8MTyFFUY";
    const SECRET: &str = "jwt_secret";

    #[test]
    fn should_succeed_to_sign_access_token() {
        let token = sign_jwt::<AccessTokenClaims>(AccessTokenClaims {
            sub: USER_ID.to_string(),
            exp: 0,
            permissions: vec![],
        }, SECRET).unwrap();
        assert_eq!(token, TOKEN);
    }
}
