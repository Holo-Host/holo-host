pub mod alias;
pub mod api_key;
pub mod api_log;
pub mod host;
pub mod job;
pub mod jurisdiction;
pub mod manifest;
pub mod metadata;
pub mod public_service;
pub mod user;
pub mod user_info;
pub mod user_permissions;
pub mod workload;

/// Name of the main database for the Holo Hosting system
pub const DATABASE_NAME: &str = "holo";

/// Parse a single key-value pair
pub fn parse_key_val<T, U>(
    s: &str,
) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}
