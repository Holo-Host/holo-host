use super::{AccessTokenClaims, RefreshTokenClaims};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;

pub fn sign_access_token(
    claims: AccessTokenClaims,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    sign_jwt::<AccessTokenClaims>(claims, secret)
}

pub fn sign_refresh_token(
    claims: RefreshTokenClaims,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    sign_jwt::<RefreshTokenClaims>(claims, secret)
}

pub fn sign_jwt<T: Serialize>(
    claims: T,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}
