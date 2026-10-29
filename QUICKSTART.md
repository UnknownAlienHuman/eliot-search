# Quickstart status

There is no verified persistent-product command sequence yet. Publishing the new documentation does not install
its target behavior. In particular, diagnostic scan examples from the unmerged source branch are not a verified
`main` interface. Do not substitute old help text, receipts or self-tests for an executable path.

## First persistent path

Follow [Foundation](docs/implementation/README.md#1-foundation--invocation-settings-and-root), then the
[one-file path](docs/implementation/README.md#2-walking-skeleton--one-file-to-one-cli-result) under
[#264](https://github.com/UnknownAlienHuman/eliot-search/issues/264):

```text
plain root + one explicit supported file
→ stable read + immutable retained bytes
→ preparation + bounded retrieval text
→ real Qdrant index and readback
→ published view + pinned evidence
→ verified excerpt in the CLI
```

The Qdrant 1.20 integration and replacement settings/protocol are selected targets, not working interfaces.
Implement their exact versions, fields and failure behavior with the actual consumers, using the owning recipes.
Do not invent a command sequence or silently fall back to the legacy local index.

After production changes run minimal locked compiler/Clippy checks for affected owners and real binaries.
Replace this status page with exact executed installation/ingestion/search commands when the connected path
works. Record the tested commit, platform, dependency versions and remaining limitations in the
[registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).
