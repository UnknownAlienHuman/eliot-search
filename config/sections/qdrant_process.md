# Qdrant connection/process configuration

This replaces the old W3 `[qdrant_process]` qualification contract. Configuration selects a live endpoint
and, optionally later, a small managed-child convenience. It does not create runtime artifact/probe receipts.

## Baseline endpoint mode

The first product needs only values consumed by the real bridge:

```toml
[qdrant]
mode = "external"
endpoint = "http://127.0.0.1:6333"
# api_key = supplied by the selected local secret/config owner when enabled
connect_timeout_ms = 5000
```

Required behavior:

- endpoint must be an explicitly selected local/allowed address;
- optional API key is an actual credential passed to the bridge, not a purpose/lease/equality record;
- startup connects and observes the current server/version/capabilities;
- publication verifies the exact selected collection generation/schema;
- static changes require restart;
- no automatic download, upgrade, fallback or provider switching.

No `expected_sha256`, executable receipt, qualification digest, restart window or artifact path is required
for external mode. Packaging/release tooling may record exact server bytes separately.

## Optional managed-child mode

Add only when a shipped standalone profile requires it:

```toml
[qdrant]
mode = "managed"
endpoint = "http://127.0.0.1:6333"
connect_timeout_ms = 5000

[qdrant.managed]
executable_path = "..."
data_dir = "..."
http_port = 6333
grpc_port = 6334
startup_timeout_ms = 30000
shutdown_timeout_ms = 10000
```

The implementation owns only the child it spawns. It may generate a private config and actual API key, but
must not expose them in argv/logs. On Windows, managed mode requires a real Job Object/child-tree platform
adapter; caller booleans/digests are not sufficient. Without that adapter, managed mode is unsupported while
external mode remains available.

First managed profile has no automatic restart. Unexpected exit makes Qdrant unavailable and preserves data;
restart policy may be added later only from a measured need.

## Parsing and application

Use the ordinary typed TOML/Serde configuration owner:

```text
parse bounded section
→ reject unknown/wrong fields
→ validate selected mode
→ construct endpoint or optional child owner
```

Do not compute a section digest, create a generic reload plan, persist a config operation receipt or call a
future qualification gate. Endpoint/managed configuration changes restart the dependency/daemon.

## Diagnostics and privacy

Diagnostics may show mode, redacted endpoint host/port, server version, connected/ready state and child exit
class. They do not expose API keys, full private paths, inherited environment, config contents or source/query
material.

## Focused checks

- external endpoint unavailable/auth failure/wrong schema;
- external mode creates no process state;
- managed missing executable/occupied endpoint/early exit/startup timeout;
- actual Windows child-tree containment or honest unsupported mode;
- bounded shutdown/reap and no orphan;
- no auto-download/upgrade/restart loop;
- unknown keys and plaintext secrets in logs/argv refused.
