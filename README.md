# holo-host main repository

![pipeline](https://github.com/holo-host/holo-host/actions/workflows/pipeline.yml/badge.svg)

This is an experiment to contain the code for all components in a single repository, also known as a monorepository.

Please run `sh setup-hooks.sh` to enforce correct naming convention for branches.

## Repository Layout

The code is grouped by language or framework name.

### Quickstart

Motivated by a shareable development experience, this repository provides

- [`nix develop .#rust`][nix develop] compatible shell environment containing a rust toolchain and other tools, including `nats` and `just`
- [`just`][just] compatible recipes via the Justfile

handily, `just` comes via the nix development shell as well.

```shell
nix develop .#rust
just
```

### Nix

```
/flake.nix
/flake.lock
/nix/ /* [blueprint](https://github.com/numtide/blueprint)  set up underneath here. */
```

### Rust

```
/Cargo.toml
/Cargo.lock
/rust/ # all rust code lives here
/rust/clients/
/rust/services/
/rust/hpos-hal/
/rust/netdiag/
/rust/util_libs/
```

### Pulumi for Infrastructure-as-Code

Reusable Pulumi modules with examples

```
/pulumi/
```

## Continuous Integration

The CI system is driven by [buildbot-nix](https://github.com/nix-community/buildbot-nix/).


## Formatting
This repo is configured with `treefmt-nix` which can be invoked via:
```
nix fmt
```


## Development Containers
The repository includes a development container environment for testing the full stack locally. This setup uses `systemd-nspawn` containers to simulate a production environment.

#### Prerequisites
- Sudo access (required for container management)
- Nix development environment using `nix develop .#rust` or `direnv allow`

### Container Components
The development environment includes:
- `dev-hub`: NATS Server (and bootstrap server for hosts into Holo system)
- `dev-orch`: Orchestrator service
- `dev-host`: Holo Host Agent
- `dev-gw`: Gateway service

## Container Networking and Port Forwarding

The container system supports two networking modes with different port forwarding approaches:

### Host Networking Mode (`privateNetwork = false`)
- Containers share the host's network namespace
- Direct port access without forwarding
- Recommended for development and testing
- No additional configuration needed

### Private Networking Mode (`privateNetwork = true`)
- Containers run in isolated network namespaces
- Requires port forwarding for external access
- Uses socat-based tunneling for reliable connectivity
- Production-ready with proper isolation

#### socat Port Forwarding Solution

Due to known reliability issues with systemd-nspawn's built-in `forwardPorts`, we implement a robust socat-based port forwarding system for private networking mode.

**What is socat?**
socat (Socket Cat) is a network utility that creates bidirectional data streams between network endpoints. It's more reliable than systemd-nspawn's port forwarding for container networking.

**How it works:**
```bash
# Creates a TCP tunnel from host port to container port
socat TCP-LISTEN:8000,fork,reuseaddr TCP:10.0.85.2:8000
```

**Implementation Details:**
- **TCP-LISTEN:8000** - Listen on port 8000 on the host
- **fork** - Create a new process for each connection
- **reuseaddr** - Allow port reuse (important for restarts)
- **TCP:10.0.85.2:8001** - Forward to container IP port 8001 (internal socat)

**Two-Tier Port Forwarding Architecture:**
The system uses a two-tier socat architecture to handle the fact that Holochain only binds to localhost inside containers:

1. **Internal socat forwarder (inside container):**
   ```bash
   # Inside container: forwards 0.0.0.0:8001 → 127.0.0.1:8000
   socat TCP-LISTEN:8001,bind=0.0.0.0,fork,reuseaddr TCP:127.0.0.1:8000
   ```
   - Bridges the gap between Holochain's localhost-only binding and container network
   - Automatically created when `privateNetwork = true`
   - Service: `socat-internal-admin-forwarder`

2. **Host-side socat tunnel (on host):**
   ```bash
   # On host: forwards localhost:8000 → container:8001
   socat TCP-LISTEN:8000,fork,reuseaddr TCP:10.0.85.2:8001
   ```
   - Provides external access from host to container
   - Connects to the internal forwarder port (8001)
   - Service: `socat-${containerName}-admin`

**Port Flow:**
```
Host Client → localhost:8000 → Host socat → 10.0.85.2:8001 → Internal socat → 127.0.0.1:8000 → Holochain
```

**Lifecycle Management:**
- socat services start after container network is ready
- Host-side services wait for internal forwarders to be active
- Network readiness detection before tunnel creation
- Clean shutdown handling with proper signal management

**Network Configuration:**
- Each container gets a unique /30 subnet: `10.0.(85+index).0/30`
- Host address: `10.0.(85+index).1`
- Container address: `10.0.(85+index).2`
- Avoids conflicts with common network ranges (Docker, VPN, etc.)

**Services Created:**
- `socat-internal-admin-forwarder` - Internal container forwarder (inside container)
- `socat-internal-httpgw-forwarder` - Internal HTTP gateway forwarder (inside container, when enabled)
- `socat-${containerName}-admin` - Host-side admin websocket port forwarding
- `socat-${containerName}-httpgw` - Host-side HTTP gateway port forwarding (when enabled)

**Advantages over systemd-nspawn forwardPorts:**
- ✅ Reliable port forwarding that actually works
- ✅ Proper lifecycle management with extra-containers
- ✅ Network-aware startup (waits for container network)
- ✅ Clean shutdown and restart handling
- ✅ Battle-tested socat for network tunneling

**Usage Example:**
```nix
# In your container configuration
privateNetwork = true;
adminWebsocketPort = 8000;
httpGwEnable = true;
httpGwPort = 8080;
```

This automatically creates a two-tier socat tunnel system:
- Host `localhost:8000` → Container `10.0.85.2:8001` → Container `127.0.0.1:8000` (admin)
- Host `localhost:8080` → Container `10.0.85.2:4000` → Container `127.0.0.1:4000` (HTTP gateway)

**Troubleshooting:**
```bash
# Check host-side socat service status (replace holochain0 with actual container name)
systemctl status socat-holochain0-admin

# Check internal container socat service status
machinectl shell holochain0 systemctl status socat-internal-admin-forwarder

# View host-side socat service logs
journalctl -u socat-holochain0-admin -f

# View internal container socat logs
machinectl shell holochain0 journalctl -u socat-internal-admin-forwarder -f

# Test end-to-end connectivity
nc -z localhost 8000

# Test container port accessibility
nc -z 10.0.85.2 8001

# Check what's listening inside container
machinectl shell holochain0 netstat -tlnp | grep ":800"

# Check container network routes
ip route show | grep "10.0.85.0/30"

# Check container firewall rules
machinectl shell holochain0 iptables -L -n | grep "8001\|8000"

# Production-specific troubleshooting:
# List all socat services (useful when container names are dynamic)
systemctl list-units --all | grep socat

# Check host-agent configuration
systemctl status holo-host-agent
journalctl -u holo-host-agent -f

# Verify environment variables
systemctl show holo-host-agent | grep Environment

# Check if required packages are installed
which socat netcat iproute2
```

#### Background: systemd-nspawn Port Forwarding Issues

The socat solution addresses well-documented reliability issues with systemd-nspawn's built-in port forwarding:

**Known Issues:**
- systemd-nspawn `forwardPorts` frequently fails to create working port mappings
- Inconsistent behavior across different systemd versions
- Poor integration with NixOS containers module
- Network timing issues during container startup

**Community Solutions:**
- [NixOS Discourse: Port forwarding of network-namespace'd containers](https://discourse.nixos.org/t/port-forwading-of-a-network-namespaced-container/54926) - Community reports port forwarding failures and suggests workarounds
- [GitHub Gist: Forward NixOS Container ports](https://gist.github.com/Saturn745/8773e3a44dc073c40600ca89027cd72e) - Documents socat-based solutions for NixOS containers
- [NixOS Issues #46975 & #28721](https://github.com/NixOS/nixpkgs/issues/46975) - Long-standing systemd-nspawn port forwarding bugs

**Why socat Works:**
- Operates at the network layer, bypassing systemd-nspawn limitations
- Mature, battle-tested network tunneling tool
- Proper integration with systemd service management
- Reliable across different system configurations
- Two-tier architecture solves localhost-only binding issues
- Handles application-specific networking constraints gracefully

**Alternative Approaches Considered:**
- iptables NAT rules (complex, fragile)
- SSH tunneling (overhead, authentication complexity)
- Host networking (loses isolation benefits)
- Custom network bridges (complex setup, maintenance overhead)

The socat approach provides the best balance of reliability, simplicity, and maintainability for production container deployments.

#### Production Deployment

**Automatic Service Creation:**
When deploying containers in production environments, the socat port forwarding services are **automatically created** without requiring additional configuration. This ensures consistent behavior between development, testing, and production environments.

**How it Works in Production:**
1. **Host Agent Deployment**: When `host_agent` creates a container with `privateNetwork = true`, it automatically includes:
   - Container configuration with internal socat services
   - Host-side socat services for external access
   - All required packages and dependencies

2. **No Manual Configuration Required**: Production systems running `holo-host-agent` with `containerPrivateNetwork = true` automatically get:
   - `socat`, `netcat-gnu`, `iproute2` packages installed
   - Complete port forwarding infrastructure
   - Proper service lifecycle management

3. **Environment Variable Control**: The `IS_CONTAINER_ON_PRIVATE_NETWORK` environment variable (set via `holo.host-agent.containerPrivateNetwork`) controls whether containers use private networking and socat port forwarding.

**Production vs Development Consistency:**
- **Test Environment**: Explicitly imports socat configuration ✅
- **Production Environment**: Gets socat configuration automatically ✅
- **Both Environments**: Identical port forwarding behavior ✅

**Deployment Example:**
```nix
# Production host configuration
holo.host-agent = {
  enable = true;
  containerPrivateNetwork = true;  # Enables automatic socat support
  # ... other configuration
};
```

When `host_agent` deploys a Holochain workload, it automatically creates:
- Container with internal socat services
- Host with external socat services  
- Complete two-tier port forwarding chain

**Verification in Production:**
```bash
# Check that containers are created with socat services
systemctl list-units | grep socat

# Verify port forwarding is working
nc -z localhost 8000

# Check container networking
machinectl list
```


### Container Packages and Platforms 
The development environment includes the following key packages or use their platform:

- **Core Tools** (required for container operation):
  - `coreutils` - Basic Unix utilities for container management
  - `systemd` - System and service manager for container orchestration
  - `bash` - Shell environment for container interaction
  - `pkg-config` - Helper tool for compiling applications and dependencies

- **NATS Stack** (required for core messaging infrastructure):
  - `nats-server` - NATS messaging server for inter-service communication
  - `natscli` - NATS command-line interface for monitoring and management
  - `nsc` - NATS configuration tool for managing NATS security

- **Database**:
  - MongoDB Atlas URL - Connection to the Holo Org's MongoDB instance

- **Development Tools**:
  - `cargo` - Rust package manager for building Rust components
  - `rustc` - Rust compiler for development
  - `just` - Command runner for development workflows
  - `holochain` binaries - Required for running Holochain tests and development


### Run the Development Environment
1. Start the development containers and follow logs:
```bash
just dev-cycle-logs

# ...or use the log compatabile version,
# if you're able to view logs with the command above
just dev-cycle-logs-compat
```

#### Test with different Holochain versions
The development environment now supports testing with different Holochain versions:

```bash
# Test with Holochain 0.5 (default - uses kitsune2 networking)
just -- dev-cycle-v05

# Test with Holochain 0.4 (legacy - uses separate bootstrap/signal services)
just -- dev-cycle-v04

# Or specify version manually
just -- dev-cycle "dev-hub dev-host dev-orch dev-gw" "0.4"
```

This will automatically:
- select the appropriate holonix package (0.3, 0.4, or 0.5)
- configure the correct bootstrap service pattern
- use compatible networking protocols

2. In a second terminal, start the Holochain terminal:
```bash
just dev-hcterm
```

3. In a third terminal, install the test application:
```bash
just dev-install-app
```

4. Switch back to the Holochain terminal and press `r` twice to refresh.


### Running an example HApp in the dev env (Humm Hive)
1. Start the development containers and follow logs:
    ```bash
    just dev-cycle-logs

    # ...or use the log compatabile version,
    # if you're able to view logs with the command above
    just dev-cycle-logs-compat
    ```
    This command:
    - Creates and starts the dev containers (dev-hub, dev-host, dev-orch, dev-gw)
    - Sets up NATS messaging infrastructure
    - Initializes the Holochain conductor
    - Starts following the logs from all services

    Example output:

    You should see logs from all services starting up, including NATS server initialization and Holochain conductor startup messages.
    ```
    [dev-hub] [INFO] Starting NATS server...
    [dev-hub] [INFO] NATS server started on port 4222
    [dev-host] [INFO] Starting Holochain conductor...
    [dev-host] [INFO] Holochain conductor started
    [dev-orch] [INFO] Orchestrator service started
    [dev-gw] [INFO] Gateway service started on port 8080
    ```

    Common errors:
    ```
    [ERROR] Failed to start NATS server: port 4222 already in use
    Solution: Run `just dev-destroy` to clean up existing containers

    [ERROR] Failed to start Holochain conductor: permission denied
    Solution: Ensure you have sudo access and run `just dev-destroy` first
    ```

2. Install the Humm Hive HApp:
    ```bash
    just dev-install-humm-hive
    ```
    This command:
    - Downloads the Humm Hive HApp bundle from the configured URL
    - Installs it into the Holochain conductor
    - Registers the HApp with the host agent
    - Starts the HApp

    Example output:
    You should see messages about the HApp being installed and started successfully.
    ```
    [INFO] Downloading HApp bundle from https://gist.github.com/steveej/...
    [INFO] Installing HApp into conductor...
    [INFO] Registering HApp with host agent...
    [INFO] Starting HApp...
    [INFO] HApp started successfully
    ```

    Common errors:
    ```
    [ERROR] Failed to download HApp bundle: network error
    Solution: Check your internet connection and try again

    [ERROR] HApp already installed
    Solution: Run `just dev-uninstall-humm-hive` first, then try installing again

    [ERROR] Failed to register with host agent: NATS connection error
    Solution: Ensure NATS server is running in dev-hub container
    ```

3.  Verify the installation:
    ```bash
    just dev-ham-find-installed-app
    ```
    This command:
    - Queries the host agent for installed applications
    - Filters for the Humm Hive HApp using the workload ID

    Example output:

    You should see the HApp details including:
    ```json
    {
    "installed_app_id": "67d2ef2a67d4b619a54286c4",
    "status": {
        "desired": "running",
        "actual": "running",
        "payload": {}
    },
    "dna_hash": "uhC0kwENLeSuselWQJtywbYB1QyFK1d-ujmFFtxsq6CYY7_Ohri2u"
    }
    ```

    Common errors:
    ```
    [ERROR] No installed app found with ID: `67d2ef2a67d4b619a54286c4`
    Solution: Ensure the hApp was installed successfully with `just dev-install-humm-hive`

    [ERROR] Failed to connect to host agent
    Solution: Check if dev-host container is running with `just dev-logs`
    ```
    
5. Option a - init without gw:
In a new terminal, initialize the Humm Hive HApp:
    ```bash
    just dev-ham-init-humm
    ```
    This command:
    - Connects to the Holochain conductor
    - Initializes the Humm Hive core zome
    - Sets up the initial Hive structure

    Example output:
    You should see a success message indicating the Hive has been initialized.
    ```
    [INFO] Connecting to Holochain conductor...
    [INFO] Initializing Humm Hive core zome...
    [INFO] Hive initialized successfully
    ```

    Common errors:
    ```
    [ERROR] Failed to connect to Holochain conductor: connection refused
    Solution: Ensure the dev containers are running with `just dev-cycle-logs`

    [ERROR] Hive already initialized
    Solution: This is not an error - the Hive can only be initialized once
    ```

Option b - init with gw
Test the HApp using the HTTP gateway:
    ```bash
    just dev-gw-curl-humm-hive
    ```
    This command:
    - Makes an HTTP request to the gateway service
    - Calls the `init` function on the `humm_earth_core` zome
    - Verifies the HApp is responding

    Example output:
    You should see a successful response from the HApp's init function.
    ```
    > GET /uhC0kwENLeSuselWQJtywbYB1QyFK1d-ujmFFtxsq6CYY7_Ohri2u/67d2ef2a67d4b619a54286c4/humm_earth_core/init
    < HTTP/1.1 200 OK
    < Content-Type: application/json
    {
    "status": "success",
    "message": "Hive initialized"
    }
    ```

    Common errors:
    ```
    < HTTP/1.1 404 Not Found
    Solution: Verify the HApp is installed and running with `just dev-ham-find-installed-app`

    < HTTP/1.1 500 Internal Server Error
    Solution: Check the gateway logs with `just dev-logs` for more details
    ```

6. Uninstall the HApp:
    ```bash
    just dev-uninstall-humm-hive
    ```
    This command:
    - Stops the HApp
    - Unregisters it from the host agent
    - Removes it from the Holochain conductor

    Example output:
    You should see confirmation messages about the HApp being stopped and uninstalled.
    ```
    [INFO] Stopping HApp...
    [INFO] Unregistering from host agent...
    [INFO] Removing from Holochain conductor...
    [INFO] HApp uninstalled successfully
    ```

    Common errors:
    ```
    [ERROR] HApp not found
    Solution: The HApp may already be uninstalled

    [ERROR] Failed to stop HApp: timeout
    Solution: Try running `just dev-destroy` to force clean up all containers
    ```

### Workload States and Flow

The development environment manages workloads through a series of states that represent the lifecycle of a workload.

Here's a description of what each state means and it's expected flow below:

1. **Initial States**:
   - `reported`: The workload has been registered and stored in mongodb, but is not yet assigned a host
   - `assigned`: The workload has been assigned to a host and has successfully stored this host in mongodb
   - `pending`: The workload has been updated in mongodb and is queued for installation on its host(s)
   - `updating`: The workload has been updated in mongodb and is queued for updating on its host(s)

2. **Installation and Update States**:
   - `installed`: The workload hApp has been installed but is not yet running
   - `updated`: The workload hApp has been successfully updated
   - `running`: The workload hApp is installed and actively running

3. **Removal States**:
   - `deleted`: The workload has been mark as deleted in mongodb and is queued for deletion on its host(s)
   - `removed`: The workload<>host links have been removed in mongodb
   - `uninstalled`: The workload hApp has been completely uninstalled from all hosts

4. **Error States**:
   - `error`: An error occurred during state transition
   - `unknown`: The current state cannot be determined


#### State Flow Example
```bash
# Initial registration and assignment (eg: just dev-install-humm-hive)
reported (stored in MongoDB) -> assigned (host stored in MongoDB) -> pending (queued/sending update install request via nats)

# Installation process
pending -> installed (hApp installed) -> running (hApp started)

# When updating (eg: just dev-hub-host-agent-remote-hc-humm)
running -> updating (queued/sending update request via nats) -> updated (hApp updated) -> running

# When uninstalling (eg: just dev-uninstall-humm-hive)
running -> deleted (marked in MongoDB) -> removed (links removed from MongoDB) -> uninstalled (hApp removed from hosts)
```

The status object in the response shows both the desired and actual states:
```json
{
    "status": {
        "desired": "running",  // The target state in MongoDB
        "actual": "running",   // The current state on the host
        "payload": {}          // Additional state-specific data (e.g., error messages, update progress)
    }
}
```

If the `actual` state differs from the `desired` state, it indicates either:
- The workload is in transition between states
- The host is still processing the state change
- An error has occurred during the state transition

#### Initial Registration and Assignment Commands

The following commands demonstrate the complete flow from initial registration to assignment:

1. **Register the workload** (reported state):
```bash
# Register a new workload in MongoDB
just dev-hub-host-agent-remote-hc reported WORKLOAD.add
```

2. **Assign the workload** (assigned state):
```bash
# Assign the workload to a host
just dev-hub-host-agent-remote-hc assigned WORKLOAD.update
```

3. **Queue for installation** (pending state):
```bash
# Queue the workload for installation
just dev-hub-host-agent-remote-hc pending WORKLOAD.insert
```

OR combine these steps into a single command:
```bash
# Register, assign, and queue in one command
just dev-install-humm-hive
```

#### Useful Commands
- View the current state in MongoDB
    ```bash
    just dev-ham-find-installed-app
    ```

- View the logs for all services
    ```
    just dev-logs
    ```

- Recreate containers and follow logs:
    ```bash
    just dev-cycle-logs
    ```

- Destroy all development containers:
    ```bash
    just dev-destroy
    ```


## Nix Tests and Checks
CI builds all Nix derivations exposed under the `checks` flake output.

While the command is called `nix build`, it's also used to execute (i.e. run) various forms of tests.

E.g., this runs the [holo-agent integration](nix/checks/holo-agent-integration-nixos.nix) test defined as NixOS VM test with verbose output:

```
nix build -vL .#checks.x86_64-linux.holo-agent-integration-nixos
```

Or this runs the [`extra-container-holochain` integration tests](nix/packages/extra-container-holochain.nix#L169), which are NixOS VM tests defined in the package file directly:

```bash
# Host networking test (recommended)
nix build -vL .#checks.x86_64-linux.pkgs-extra-container-holochain-integration-host-network

# Private networking test (documents port forwarding issues)
nix build -vL .#checks.x86_64-linux.pkgs-extra-container-holochain-integration-private-network
```

### Test Environment Requirements

The test environment automatically provides:
- MongoDB for database tests
- NATS server for messaging tests
- Systemd for service management
- Filesystem tools for storage tests
- Network isolation for integration tests


The testing environment includes additional packages and tools:
- **Database**:
  - `mongodb-ce` - MongoDB Community Edition (used for running integration tests)

- **Filesystem Tools** (for hpos-hal testing):
  - `dosfstools` - Tools for FAT filesystems
  - `e2fsprogs` - Tools for ext2/ext3/ext4 filesystems


### Rust-specific Checks
1. **Clippy Linting**:
```bash
cargo fmt && cargo clippy
```
Runs Rust's linter and clippy to catch common mistakes and enforce style guidelines.


### System Integration Tests
1. **Holo Agent Integration**:
```bash
nix build -vL .#checks.x86_64-linux.holo-agent-integration-nixos
```
Runs a NixOS VM test that:
- Sets up a complete Holo agent environment
- Tests agent initialization
- Verifies agent communication
- Tests workload management

2. **Holochain Container Integration**:

   **Host Networking Test** (recommended - works reliably):
   ```bash
   nix build -vL .#checks.x86_64-linux.pkgs-extra-container-holochain-integration-host-network
   ```
   
   **Private Networking Test** (currently failing due to systemd-nspawn port forwarding compatibility):
   ```bash
   nix build -vL .#checks.x86_64-linux.pkgs-extra-container-holochain-integration-private-network
   ```
   
   Both tests verify:
   - Container creation and initialization
   - Holochain conductor configuration
   - Service readiness with systemd notifications
   - Network connectivity (host vs private networking)
   - Environment variable handling for `IS_CONTAINER_ON_PRIVATE_NETWORK`
   - State persistence (holochain data directory and configuration)


### Running Tests Locally
```bash
# Run Rust tests
cargo test

# Run integration tests
nix build -vL .#checks.x86_64-linux.holo-agent-integration-nixos
```

## Licenses
Please see the [LICENSE](./LICENSE) file.

[just]: https://just.systems/man/en/
[nix develop]: https://zero-to-nix.com/concepts/dev-env/

# Check if the environment variable is set correctly in the host agent
sudo systemctl show holo-host-agent.service | grep Environment | grep CONTAINER

# Or check the service environment directly
sudo systemctl cat holo-host-agent.service | grep -A10 -B10 CONTAINER