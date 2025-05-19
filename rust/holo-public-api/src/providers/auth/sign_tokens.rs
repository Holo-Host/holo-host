use crate::providers::jwt::{
    sign_access_token, sign_refresh_token, AccessTokenClaims, RefreshTokenClaims,
};

pub struct SignJwtTokenOptions {
    pub jwt_secret: String,
    pub access_token: AccessTokenClaims,
    pub refresh_token: RefreshTokenClaims,
}

pub fn sign_tokens(options: SignJwtTokenOptions) -> Option<(String, String)> {
    let access_token = match sign_access_token(options.access_token, options.jwt_secret.clone()) {
        Ok(claims) => claims,
        Err(_err) => {
            tracing::error!("failed to sign access token");
            return None;
        }
    };
    let refresh_token = match sign_refresh_token(options.refresh_token, options.jwt_secret) {
        Ok(token) => token,
        Err(_err) => {
            tracing::error!("failed to sign refresh token");
            return None;
        }
    };
    Some((access_token, refresh_token))
}
