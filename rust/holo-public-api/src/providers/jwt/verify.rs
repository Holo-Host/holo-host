use super::claims::{AccessTokenClaims, RefreshTokenClaims};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::de::DeserializeOwned;

pub fn verify_access_token(
    token: &str,
    secret: &str,
) -> Result<AccessTokenClaims, jsonwebtoken::errors::Error> {
    verify_jwt::<AccessTokenClaims>(token, secret)
}

pub fn verify_refresh_token(
    token: &str,
    secret: &str,
) -> Result<RefreshTokenClaims, jsonwebtoken::errors::Error> {
    verify_jwt::<RefreshTokenClaims>(token, secret)
}

pub fn verify_jwt<T: DeserializeOwned>(
    token: &str,
    secret: &str,
) -> Result<T, jsonwebtoken::errors::Error> {
    let mut validation = Validation::default();
    validation.validate_exp = false;
    let token = decode::<T>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    );

    match token {
        Ok(token) => Ok(token.claims),
        Err(e) => Err(e),
    }
}
