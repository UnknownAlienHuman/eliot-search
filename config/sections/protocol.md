# Protocol configuration

Protocol behavior belongs to `search-provider-protocol`; the daemon acquires settings and owns the
connection. Follow the [local protocol implementation](../../docs/implementation/README.md#queries-windows-and-process-lifetime).
This replaces the former packet's universal pairing requirement and generic section-digest/receipt API.

## Current implementation

The existing V1 [binding API](../../crates/search-provider-protocol/src/binding.rs) requires
`VerifiedPairing` in `authenticate_binding` and `BoundSession::open`. Its authenticated envelopes
and callers have not been converted to the trusted-local target.

The [kernel defaults](../../crates/search-provider-protocol/src/config.rs) are:

| Runtime setting | Implemented value/bound |
|---|---|
| `ProtocolLimits.max_frame_bytes` | At most 8,388,608 bytes for the complete frame, including its four-byte prefix. |
| `ProtocolLimits.max_body_bytes` | Default 8,388,604; cannot exceed frame capacity minus the prefix. |
| `ProtocolLimits.max_in_flight_requests` | Default 32; accepted range 1–32. |
| `ProtocolLimits.max_replay_entries` | Default 4,096; must be nonzero in the current session API. |
| Compression/fragment assembly | Not implemented by the framing codec. |

The historical configuration reference/example declared `pairing_required = true`, hello timeouts and other
section fields. The [current source audit](../../docs/config/CONFIGURATION_1.0.md) replaces that catalogue and
the [current example](../eliot-search.example.toml) is only a parser-compatible no-op. The old table was not an
implemented settings API: `ProtocolLimits` has no
`pairing_required` field, and the current
[daemon registry](../../bins/eliot-searchd/src/config_composition/kernel/registry.rs) does not register
`[protocol]`. Setting `pairing_required = false` therefore does not enable a supported local mode.
The enforced pairing constraint is in the existing session constructors and consumers.

## Target change

Use owned child stdio or an owner-only local pipe for the trusted-local exchange. Remove mandatory
pairing, body MAC and grant inputs from that API with its actual client/daemon callers. Change the
settings registry/parser, examples and session construction in the same integration; do not advertise
an unsupported flag or pass invented proofs to the old API. One current protocol replaces the old
forms without a compatibility adapter or parallel endpoint.

For a selected restricted-client profile, enforce actual credentials, replay protection and delegated
scope. A generic configuration switch must not silently weaken that profile's promised restrictions.

Keep finite input/output, queue and in-flight limits. Check the declared frame length before allocating
or reading its body; use typed decoding after framing. The runtime converts the admitted request limit
to one absolute deadline used by all operations. A settings change does not replenish running requests'
budgets. Unsupported compression or fragmented-message settings remain errors.

Only expose settings that the current application consumes. Add clear defaults, type/range validation
and actionable errors with those consumers; do not require a section fingerprint, generic reload
receipt or delivery-wave approval for each value. Existing connections retain coherent settings until
replaced; a new transport format is selected when a connection is opened.

## Verification

Check actual configuration-to-runtime values, malformed/unknown options, prefix and output bounds,
EOF/cancellation cleanup and the selected connection profile. Restricted-client checks apply when that
profile is used. A parsed document or a section digest does not demonstrate that the daemon applies it.
