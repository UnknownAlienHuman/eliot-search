# Ownership and dependency boundary audit

**Audited base:** `08ce83397c0121733c0d6fb95d9bc88f65fe71bf`  
**Architecture:** ELIOT Search 8.4  
**Result:** scaffold hardening required; business implementation remains absent.

## Closed findings

| ID | Severity | Finding | Closure |
|---|---:|---|---|
| F-01 | Blocker | `expand_handle@1`, durable eligibility, expansion authorization and revocation had no mutable-state owner. | Added `search-handles`; projector only requests handles and continuation keeps only continuation state. |
| F-02 | Blocker | C17 computed watermarks but no package owned exact retired-point deletion. | Added `search-index-reclaimer`; committed exact IDs plus pin watermark are mandatory. |
| F-03 | Major | Qdrant process containment and vendor data plane were conflated. | Added `search-qdrant-supervisor`; bridge now owns data plane only. |
| F-04 | Major | OS-bound secret storage was implicit in daemon/provider code. | Added `search-os-secrets` with opaque references and side-channel tests. |
| F-05 | Major | Admission policy was duplicated across registry and safe reader. | Added pure `search-source-admission`; registry verifies receipts and reader performs no policy decision. |
| F-06 | Blocker | Query/lifecycle packages declared direct concrete adapter dependencies. | Replace those edges with vendor-neutral ports; concrete adapters are composed only by `eliot-searchd`. |
| F-07 | Major | Ordinary reclaim, security purge and CAS retention could collapse into one deletion path. | Ownership is explicit: reclaimer = retired index points; retention = CAS/purge/restore; purge acknowledgements are distinct. |

## Deliberate boundaries retained

- Publication commit and crash recovery remain one linearizable state-machine owner.
- Exact plan compilation/execution remain one proof owner.
- CAS mark/sweep, purge and restore remain one monotonic lifecycle-policy owner.
- Filesystem/Git stable read stays one baseline package until native dependencies or size prove a split.

## Honest status

```text
architecture coherence: reviewed
package ownership: refined
business Rust implementation: absent
runtime/fault/security tests: not executed
Qdrant/Windows qualification: not executed
performance evidence: absent
product acceptance: not accepted
current implementation authorization: P00/W0 only
```
