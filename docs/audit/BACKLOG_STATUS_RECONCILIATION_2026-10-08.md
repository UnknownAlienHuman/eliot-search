# Backlog and status reconciliation — 2026-10-08

**Audited source:** `3ccef79a6850a9fd4caa7941c40e4a331cf4e0dc`  
**Coordinator:** #97  
**Documentation owner:** #222

## Result

The early P00/P01/P02/W0/W1/W2 issue wave duplicated package work that is already present or has a
new exact owner. Issues `#51–#89` were closed with `state_reason=not_planned` and an individual
successor comment. This means **superseded**, not completed or qualified.

Issue `#48` is closed as completed. Verification against the active `TYPE_REGISTRY.md` confirmed exact
named entries for `UtcTimestamp`, `MetadataKey` and `UnresolvedSource`; the stale handoff note, not the
normative registry, was incorrect.

## Successor map

| Area | Current owner |
|---|---|
| Canonical codec/digests | #237 |
| Configuration | #238 → #109 |
| Source admission | #241 → #110 |
| Source identity/registry/currentness | #110 → #128 → #218 |
| Provider client and product transport | #235 → #116 |
| Control redb | #106 → #108 |
| Runtime/data root/native boundaries | #100/#104/#105/#120 |
| Secrets | #115/#120 |
| Safe reader/Git | #104/#129 |
| Revision/retention/restore | #111/#134/#243 → #244 → #245 |
| Materialization/documents | #112/#113/#216/#226/#227 |
| Installed qualification | #240 → #215 |

## Closed issues

```text
#51 #53 #54 #55 #56 #59 #60 #61 #63 #64 #65
#68 #69 #70 #71 #72 #73 #74 #75 #76 #77 #78
#79 #80 #81 #82 #83 #84 #85 #86 #87 #88 #89
```

Other early issues already closed before this reconciliation are not reopened.

## Documentation contradiction

At the audited source revision, 26 active README files still contain the obsolete sentence
`behavior is intentionally unimplemented`, including domain, ports, query, source, lifecycle,
Qdrant and edge packages with substantive source.

#222 must replace active stale wording with evidence-bounded status and point every workspace member
to `docs/product/PACKAGE_STATUS.toml`. Historical scaffold documents may retain old wording only when
clearly archived and non-authoritative.

## Evidence boundary

This reconciliation changes task authority and documentation planning only. It does not assert a
fresh Cargo, Clippy, test, Windows, Qdrant, fault, scale or installed-product PASS.
