# Distributed Authentication Test: Walkthrough & Production Alignment

---

## Overview

This document explains the `holo-distributed-auth.nix` test, how it simulates the production authentication flow for Holo-Host’s NATS/Orchestrator system, and what aspects of the real system it covers.

---

## 1. Test Architecture & Flow

### **High-Level Flow**

```mermaid
flowchart TD
  subgraph NATS_Server_VM [NATS Server VM]
    A1["holo-nats-auth-setup.service\n(NSC credential generation)"]
    A2["nats-shared-creds-copy.service\n(Copy creds to /tmp/shared)"]
    A3["nats-server.service\n(NATS server startup)"]
    A1 --> A2 --> A3
  end

  subgraph Orchestrator_VM [Orchestrator VM]
    B1["Activation script\n(Copy creds from /tmp/shared)"]
    B2["holo-orchestrator.service\n(Orchestrator startup)"]
    B1 --> B2
  end

  A2 -- "creds via /tmp/shared" --> B1
  A3 -- "NATS running" --> B2
```

- **NATS Server VM**: Generates NSC credentials, shares them, and starts NATS.
- **Orchestrator VM**: Receives credentials, starts orchestrator with real creds.
- **/tmp/shared/**: Simulates secure cross-node credential transfer.

---

## 2. Step-by-Step Test Walkthrough

### **A. NSC Credential Generation**
- `holo-nats-auth-setup.service` runs on the NATS server VM.
- Uses NSC CLI to generate:
  - Operator JWT (`HOLO.jwt`)
  - System account JWT (`SYS.jwt`)
  - Admin user creds (`admin_user.creds`)
  - Orchestrator user creds (`orchestrator_auth.creds`)
- Places all in `/var/lib/nats_server/shared-creds/`.

### **B. Credential Sharing**
- `nats-shared-creds-copy.service` (runs as root):
  - Copies `admin_user.creds` and `orchestrator_auth.creds` to `/tmp/shared/`.
  - Only runs after NSC credentials are generated.

### **C. Orchestrator Credential Consumption**
- Orchestrator VM activation script:
  - Copies creds from `/tmp/shared/` to `/var/lib/holo-orchestrator/nats-creds/`.
  - Orchestrator service uses these to connect to NATS.

### **D. Service Startup Order**
- NATS server starts only after credentials are ready.
- Orchestrator starts only after NATS and credentials are ready.

---

## 3. Test Script: Verification Steps

```mermaid
flowchart TD
  subgraph Test_Steps [Test Script Steps]
    T1["Check NATS service status"]
    T2["Check NATS shared-creds dir and file content"]
    T3["Check /tmp/shared/ on both VMs"]
    T4["Check orchestrator nats-creds dir and file content"]
    T5["Check orchestrator config/mongo files"]
    T6["Check orchestrator service status"]
    T7["Check file content: head/cat"]
    T8["Final success marker"]
    T1 --> T2 --> T3 --> T4 --> T5 --> T6 --> T7 --> T8
  end
```

- **Checks NATS and orchestrator service status**
- **Lists and inspects all relevant credential files**
- **Verifies file existence, non-emptiness, and valid JWT content**
- **Checks shared directory on both VMs**
- **Checks orchestrator config files (e.g., Mongo creds)**
- **Prints actual file content for human verification**
- **Ends with a clear success marker if all pass**

---

## 4. Test Coverage

```mermaid
flowchart TD
  subgraph Coverage [Test Coverage]
    C1["NSC credential generation (admin, orchestrator)"]
    C2["Credential sharing via /tmp/shared/"]
    C3["NATS server startup with JWT auth"]
    C4["Orchestrator startup with real creds"]
    C5["File existence and content checks"]
    C6["Service dependency and order"]
    C7["No mocks: only real NSC creds"]
    C8["Permissions and ownership checks"]
    C1 --> C2 --> C3 --> C4 --> C5 --> C6 --> C7 --> C8
  end
```

### **What’s Covered?**
- **End-to-end credential lifecycle:** Generation, sharing, consumption.
- **Service startup order and dependencies.**
- **File presence, permissions, and content.**
- **No mocks: Only real NSC-generated credentials are used.**
- **Simulates real distributed, multi-node production deployment.**

---

## 5. Alignment with Production Auth Flow

| Test Step | Production Equivalent | Purpose |
|-----------|----------------------|---------|
| NSC credential generation | NSC CLI run by ops/automation | Ensures real JWTs/creds are created |
| Copy to /tmp/shared/ | Secure transfer (e.g., secrets manager, volume) | Simulates cross-node sharing |
| Orchestrator activation script | Secure credential mount/transfer | Simulates orchestrator receiving creds |
| Service dependencies | Systemd/infra dependencies | Prevents race conditions |
| File/content checks | Health checks, monitoring | Ensures system is ready and secure |

---

## 6. Conclusion

- **This test is a faithful simulation of the production distributed authentication flow.**
- **All critical steps are covered, with real credentials and real service startup order.**
- **If this test passes, you can be confident the production flow will work.**

---

*For further details, see the test script in `holo-host/nix/checks/holo-distributed-auth.nix`.* 