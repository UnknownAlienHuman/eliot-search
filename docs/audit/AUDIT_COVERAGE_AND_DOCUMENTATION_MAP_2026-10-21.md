# Historical coverage map — superseded

The former “static audit closure” claim was too broad. Later source/document passes found additional defects,
stale callers and conflicting instructions. This file remains only as a stable target for old links; it is not
current status, a completion gate or an alternative specification.

The complete pre-correction text is preserved at
[`be84c8ab`](https://github.com/UnknownAlienHuman/eliot-search/blob/be84c8abb395e2b96426cf7b1bb87a5cae4d062d/docs/audit/AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md).
Its 52-member inventory and 288-path sweep describe earlier snapshots.

Use these current records:

| Question | Current location |
|---|---|
| Product purpose and requirements | [Architecture](../architecture/README.md) |
| Selected mechanisms and owners | [Implementation](../implementation/README.md#owners) and nearest `FUNCTIONS.md` |
| Current checkpoint, area status and execution evidence | [Analysis registry](ANALYSIS_REGISTRY.md) |
| Full prior passes and exact revisions | [Evidence index](ANALYSIS_REGISTRY.md#evidence-index) |
| Donor versions and applicability limits | [Donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md) |
| Baseline package/test disposition | [Workspace map](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md), [test cutover](../implementation/TEST_CUTOVER.md) |

Neither an audit label nor a proposed fix establishes a working application, compiler success or runtime
behavior. Fix known defects directly; do not require another repository-wide audit before implementation.
