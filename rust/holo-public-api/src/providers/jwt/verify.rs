use super::claims::{AccessTokenClaims, RefreshTokenClaims};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::de::DeserializeOwned;

pub fn verify_access_token(
    token: String,
    secret: String,
) -> Result<AccessTokenClaims, jsonwebtoken::errors::Error> {
    verify_jwt::<AccessTokenClaims>(token, secret)
}

pub fn verify_refresh_token(
    token: String,
    secret: String,
) -> Result<RefreshTokenClaims, jsonwebtoken::errors::Error> {
    verify_jwt::<RefreshTokenClaims>(token, secret)
}

pub fn verify_jwt<T: DeserializeOwned>(
    token: String,
    secret: String,
) -> Result<T, jsonwebtoken::errors::Error> {
    let mut validation = Validation::default();
    validation.validate_exp = false;
    let token = decode::<T>(
        token.as_ref(),
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    );

    match token {
        Ok(token) => Ok(token.claims),
        Err(e) => Err(e),
    }
}
