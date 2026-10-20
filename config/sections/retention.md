# Retention and GC settings disposition

`[retention]` is not registered by the current daemon. The generic retention/lease/graph/sweep/purge/restore
platform is being deleted.

Current-format storage keeps objects reachable from current source/prepared state, pending publication,
current/live views and finite cleanup artifacts. One process-local GC gate under the exclusive root lock closes
the finalize-to-first-reference race. Unknown/corrupt/incomplete roots retain bytes.

Publication separately owns Qdrant cleanup. Legal hold, security purge and secure-erase claims are not config
toggles. Add private batch limits only when the concrete GC owner consumes them; no retention section digest,
security-barrier receipt or completed sweep journal.
