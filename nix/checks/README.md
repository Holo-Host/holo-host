# Nix Checks

This directory contains NixOS integration tests that verify the functionality of various components in a controlled environment, with a focus on the distributed authentication flow.

## Available Checks

### `holo-agent-integration-nixos.nix`
Tests the integration between the holo-host-agent and NATS server, including workload management and communication. This test now includes proper distributed authentication setup with JWT resolver configuration.

**Features:**
- NATS server with distributed auth setup (HOLO operator, ADMIN/AUTH/HPOS accounts)
- JWT resolver configuration for secure authentication
- Host agent connectivity testing
- Workload stream management
- JetStream domain configuration

### `holo-nsc-proxy.nix`
Tests the NSC proxy server functionality with distributed authentication, including:
- NSC proxy server startup and health checks
- Authentication and authorization
- Command validation and execution
- Firewall rule enforcement
- Orchestrator integration with distributed auth

**Features:**
- NATS server with distributed auth setup (Part 1)
- Orchestrator with distributed auth setup (Part 2)
- NSC proxy integration testing
- User creation via NSC proxy
- Credential generation and storage
- Security verification (permissions, ownership)

### `holo-distributed-auth.nix`
**NEW**: Comprehensive test for the distributed authentication flow between NATS server and orchestrator.

**Features:**
- Complete distributed auth pattern implementation
- NATS server auth setup (Part 1: operator, accounts, signing keys)
- Orchestrator auth setup (Part 2: local user keys, NSC proxy integration)
- Security verification (file permissions, ownership, key separation)
- Error handling and validation testing
- Service connectivity verification

## Distributed Authentication Flow

The tests implement the 3-part distributed authentication architecture:

### Part 1: NATS Server Setup
- Creates HOLO operator with signing keys
- Creates ADMIN, AUTH, and HPOS accounts
- Sets up export/import rules between accounts
- Extracts signing keys for local storage
- Generates resolver configuration
- Creates shared credentials for host agents

### Part 2: Orchestrator Setup
- Generates local user keys using `openssl rand -hex 32`
- Creates users via NSC proxy (admin, orchestrator_auth)
- Generates credentials via NSC proxy
- Stores credentials locally with proper permissions
- Maintains key separation (orchestrator owns user keys)

### Part 3: Security Verification
- Verifies file permissions (600 for credentials)
- Checks ownership (nats-server vs orchestrator)
- Validates key separation
- Tests error handling and validation
- Verifies service connectivity

## Running Checks

### Run All Checks
```bash
cd holo-host
nix flake check
```

### Run Specific Check
```bash
# NSC Proxy test
nix build .#checks.x86_64-linux.holo-nsc-proxy

# Host Agent Integration test
nix build .#checks.x86_64-linux.holo-agent-integration-nixos

# Distributed Auth test
nix build .#checks.x86_64-linux.holo-distributed-auth
```

### Run All Auth Tests
```bash
nix build .#checks.x86_64-linux.holo-nsc-proxy .#checks.x86_64-linux.holo-distributed-auth
```

## Test Environment

### NSC Proxy Test
Creates a virtual network with two nodes:
- **nats-server**: Runs NATS server with NSC proxy enabled and distributed auth setup
- **orchestrator**: Runs the orchestrator service that connects to the NSC proxy

### Host Agent Integration Test
Creates a virtual network with multiple nodes:
- **hub**: NATS server with distributed auth and JWT resolver
- **host1-host5**: Host agents with local NATS servers

### Distributed Auth Test
Creates a virtual network with two nodes:
- **nats-server**: Complete distributed auth setup (Part 1)
- **orchestrator**: Orchestrator with distributed auth setup (Part 2)

## Test Verification

The tests verify:

1. **Service Startup and Health**
   - NATS server with JWT resolver
   - NSC proxy server
   - Orchestrator service
   - Host agent services

2. **Authentication Setup**
   - Operator and account creation
   - Signing key extraction
   - User creation via NSC proxy
   - Credential generation

3. **Security Aspects**
   - File permissions (600 for credentials)
   - Proper ownership
   - Key separation
   - Firewall rules

4. **API Functionality**
   - NSC proxy health endpoint
   - Command validation
   - Error handling
   - Authentication rejection

5. **Integration**
   - Service connectivity
   - Message passing
   - Stream management
   - Workload handling

## Debugging

To debug a failing test, you can run it interactively:

```bash
cd holo-host
nix build .#checks.x86_64-linux.holo-distributed-auth --rebuild
```

Check the test output for detailed error messages and logs from each service.

## Architecture Compliance

The tests ensure compliance with the distributed authentication architecture:

- **Key Separation**: NATS server owns operator/account keys, orchestrator owns user keys
- **Network Security**: NSC proxy with authentication and firewall rules
- **Credential Management**: Local storage with proper permissions
- **Service Dependencies**: Proper startup order and connectivity
- **Error Handling**: Comprehensive validation and rejection testing

## Migration Notes

The updated tests replace the old authentication patterns with the new distributed approach:

- **Old**: Single-script auth setup with direct NSC access
- **New**: 3-part distributed auth with NSC proxy integration
- **Security**: Improved key separation and local credential storage
- **Testing**: Comprehensive verification of the distributed pattern 