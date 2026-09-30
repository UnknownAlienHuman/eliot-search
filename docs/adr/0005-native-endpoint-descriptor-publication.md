# ADR 0005 — Publish one authenticated native endpoint descriptor

- **Status:** proposed
- **Date:** 2026-09-30
- **Scope:** standalone daemon/client bootstrap on one local data root
- **Architecture:** ELIOT Search 8.4 S27, S29-S30.3, S32-S33; P00 provider lifecycle; W8 client edge

## Context

The standalone CLI must reach the daemon through the canonical provider protocol without opening the
control journal, scanning ports, reading a plaintext token file or depending on an operating-system
secret adapter. The daemon already owns the data-root lock, durable provider registration, exact
registered-generation credential and loopback listener. The client already accepts a bounded
`NativeEndpointDescriptorV1`, independently checks expected registration coordinates and asks an
injected platform owner for the credential selected by the canonical locator digest.

A raw address file is insufficient. Any same-user process can create or replace ordinary local files,
and an old file can survive a crash. Filesystem location therefore cannot authenticate the endpoint or
grant authority. Conversely, putting a secret in the file would create a second credential store and
violate the W8 client boundary.

## Decision

The root-owning daemon publishes exactly one public descriptor at:

```text
<data-root>/runtime/native-endpoint.v1
```

Publication occurs only after durable standalone registration finalization and successful IPv4
loopback bind. The descriptor contains the actual nonzero port and the exact installation,
incarnation, binding, peer-identity digest, pairing generation, selected profile, disclosure-policy
reference, pairing reference, optional requested-capability digest and protocol version defined by
`NativeEndpointDescriptorV1`.

The daemon loads the existing credential for that exact registered generation and computes the keyed
proof over the canonical descriptor transcript. It never generates, adopts or replaces a credential
while publishing the endpoint. The file contains no key, token, grant, source permit or mutable
authority.

The listener retains the published pairing reference and requested-capability digest. The first
canonical `Hello` must match both values before the daemon reads the pairing key or emits a challenge.
The normal mutual-proof ceremony, current binding read, policy checks and typed-profile handshake
remain mandatory.

## Publication and lifetime rules

- `runtime` and the final descriptor must remain under the canonical root and must not be symlinks,
  reparse points or non-regular objects.
- Descriptor bytes are bounded by the protocol ceiling.
- Writes use a same-directory `create_new` temporary file, content sync, atomic replacement, directory
  durability where the safe standard library exposes it, and exact bounded readback.
- One caller `OperationContext` supplies a diminishing deadline and cancellation capability across
  listener bind completion, credential read, signing, publication and readback.
- Listener construction returns no admission capability unless publication succeeds.
- Field/drop order closes listener admission first, then removes only the byte-identical descriptor,
  then releases the process and data-root owner.
- A changed descriptor is never deleted as cleanup. A crash-stale valid descriptor is harmless by
  itself: the client still needs the independently selected credential and the server at that address
  must complete mutual proof.

The client continues to require independently trusted registration coordinates and a platform-owned
key source. A descriptor alone never chooses its own trust root.

## Rejected alternatives

- **Port scanning or fixed guessed ports:** unauthenticated discovery and ambiguous ownership.
- **Plaintext token/key file:** second secret store, copyable authority and W8 violation.
- **Direct Credential Manager dependency in `eliot-search`:** forbidden concrete adapter edge.
- **Descriptor fields accepted only from `Hello`:** lets the peer alter coordinates after descriptor
  verification.
- **Delete-on-cleanup without exact byte comparison:** can remove another live owner's publication.

## Consequences

The daemon composition root gains one bounded runtime artifact and its lifecycle owner. Protocol and
credential formats do not change. Stale or malicious files cause refusal, not fallback. Default CLI
routing, concrete platform key-source composition, trusted production startup inputs and recipe hosts
remain separate work.

This ADR does not claim T19 or T20 acceptance.

## Required qualification before acceptance

- compile and Clippy for `eliot-searchd` on Windows and Linux;
- Windows Credential Manager success, missing, conflict and cancellation paths;
- exact descriptor encode/decode/proof interoperability between daemon and CLI;
- ephemeral and fixed loopback ports;
- corrupt, oversized, truncated, symlink/reparse and root-escape descriptor objects;
- replacement acknowledgement loss and exact recovery readback;
- crash-stale descriptor followed by restart and replacement;
- shutdown removal, changed-file cleanup conflict and listener-before-root drop order;
- `Hello` pairing-reference and capability-digest mismatch before key access;
- proof that no secret bytes, raw paths or platform credential handles enter diagnostics.
