# Auth

## JWT

### Access Token

The access token is a JWT token that contains the user's claims.
Access tokens are short lived and are used to access resources.

#### Claims

The access token contains the following claims:

permissions: `Vec<String>` - The permissions of the user

sub: `String` - The user's id

exp: `usize` - The expiration time of the token

### Refresh Token

The refresh token is a JWT token that contains the user's claims.
Refresh tokens are long lived and are used to refresh the access token.

#### Claims

The refresh token contains the following claims:

sub: `String` - The user's id

exp: `usize` - The expiration time of the token

version: `i32` - The version of the token

## Permissions

The permissions are a list of strings that represent the user's permissions.
If a permission ends with `_ALL`, it means the user has access to all resources.
Otherwise, the user only has access to resources where the owner is the same as the user.

### List of Permissions

- `WORKLOADS_READ` - Read workloads
- `WORKLOADS_WRITE` - Write workloads
- `WORKLOADS_DELETE` - Delete workloads
- `WORKLOADS_READ` - Read workloads
- `WORKLOADS_WRITE` - Write workloads

Q? should we add update and delete or make write include update and delete?