# Product behavior

This directory contains product-facing behavior, not package status, launch authority or an agent control plane.

- [Agent analysis framework](AGENT_ANALYSIS_FRAMEWORK.md) describes the user-visible navigation, evidence, comparison and exact-verification experience that the rewrite must preserve.
- [Architecture](../architecture/README.md) defines the product boundary and principles.
- [Implementation](../implementation/README.md) selects the current mechanisms and owners.

`PACKAGE_STATUS.toml`, implementation-status queues and execution protocols from Architecture 8.4 were removed. Git history is their archive. Do not recreate a machine-readable package/wave/status graph: source presence, closed tasks and structural checks do not prove connected behavior.

Current coordination is issue #352 plus the concrete owner issue/PR. Package boundaries may change; user requirements do not disappear merely because a broken package is deleted.
