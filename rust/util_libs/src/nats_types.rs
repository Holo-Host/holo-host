/* --------
NOTE: These types are the standaried types from NATS and are already made available as rust structs via the `nats-jwt` crate.
IMP: Currently there is an issue serizialing claims that were generated without any permissions. This file removes one of the serialization traits that was causing the issue, but consequently required us to copy down all the related nats claim types.
TODO: Make PR into `nats-jwt` repo to properly fix the serialization issue with the Permissions Map, so we can import these structs from thhe `nats-jwt` crate, rather than re-implmenting them here.
-------- */

use serde::{Deserialize, Serialize};

/// JWT claims for NATS compatible jwts
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Time when the token was issued in seconds since the unix epoch
    #[serde(rename = "iat")]
    pub issued_at: i64,

    /// Public key of the issuer signing nkey
    #[serde(rename = "iss")]
    pub issuer: String,

    /// Base32 hash of the claims where this is empty
    #[serde(rename = "jti")]
    pub jwt_id: String,

    /// Public key of the account or user the JWT is being issued to
    pub sub: String,

    /// Friendly name
    pub name: String,

    /// NATS claims
    pub nats: NatsClaims,

    /// Time when the token expires (in seconds since the unix epoch)
    #[serde(rename = "exp", skip_serializing_if = "Option::is_none")]
    pub expires: Option<i64>,
}

/// NATS claims describing settings for the user or account
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NatsClaims {
    /// Claims for NATS users
    User {
        /// Publish and subscribe permissions for the user
        #[serde(flatten)]
        permissions: NatsPermissionsMap,

        /// Public key/id of the account that issued the JWT
        issuer_account: String,

        /// Maximum nuber of subscriptions the user can have
        subs: i64,

        /// Maximum size of the message data the user can send in bytes
        data: i64,

        /// Maximum size of the entire message payload the user can send in bytes
        payload: i64,

        /// If true, the user isn't challenged on connection. Typically used for websocket
        /// connections as the browser won't have/want to have the user's private key.
        bearer_token: bool,

        /// Version of the nats claims object, always 2 in this crate
        version: i64,
    },
    /// Claims for NATS accounts
    Account {
        /// Configuration for the limits for this account
        limits: NatsAccountLimits,

        /// List of signing keys (public key) this account uses
        #[serde(skip_serializing_if = "Vec::is_empty")]
        signing_keys: Vec<String>,

        /// Default publish and subscribe permissions users under this account will have if not
        /// specified otherwise
        /// default_permissions: NatsPermissionsMap,
        ///
        /// Version of the nats claims object, always 2 in this crate
        version: i64,
    },
}

/// List of subjects that are allowed and/or denied
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NatsPermissions {
    /// List of subject patterns that are allowed
    /// #[serde(skip_serializing_if = "Vec::is_empty")]
    /// ^^ causes the serialization to fail when tyring to seralize raw json into this struct...
    pub allow: Vec<String>,

    /// List of subject patterns that are denied
    /// #[serde(skip_serializing_if = "Vec::is_empty")]
    /// ^^ causes the serialization to fail when tyring to seralize raw json into this struct...
    pub deny: Vec<String>,
}

impl NatsPermissions {
    /// Returns `true` if the allow and deny list are both empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.allow.is_empty() && self.deny.is_empty()
    }
}

/// Publish and subcribe permissons
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NatsPermissionsMap {
    /// Permissions for which subjects can be published to
    #[serde(rename = "pub", skip_serializing_if = "NatsPermissions::is_empty")]
    pub publish: NatsPermissions,

    /// Permissions for which subjects can be subscribed to
    #[serde(rename = "sub", skip_serializing_if = "NatsPermissions::is_empty")]
    pub subscribe: NatsPermissions,
}

/// Limits on what an account or users in the account can do
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsAccountLimits {
    /// Maximum nuber of subscriptions the account
    pub subs: i64,

    /// Maximum size of the message data a user can send in bytes
    pub data: i64,

    /// Maximum size of the entire message payload a user can send in bytes
    pub payload: i64,

    /// Maxiumum number of imports for the account
    pub imports: i64,

    /// Maxiumum number of exports for the account
    pub exports: i64,

    /// If true, exports can contain wildcards
    pub wildcards: bool,

    /// Maximum number of active connections
    pub conn: i64,

    /// Maximum number of leaf node connections
    pub leaf: i64,
}
