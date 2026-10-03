# Qdrant diagnostic observations — 2026-10-03

These files were retained from a repository execution detour because they contain
Search-specific observations against Qdrant 1.19.0 on Windows. They are diagnostic
inputs only.

They do **not** constitute W3 qualification, do not enable indexed mode, do not
select an artifact/profile, and do not prove the Rust bridge or product spine.
The JSON records preserve their own PASS/FAIL and `UNQUALIFIED` dispositions.

Retained observations:

- `qdrant-auth-crud-restart.json` — authenticated CRUD, exact readback, count,
  restart persistence and deletion on a disposable collection;
- `qdrant-short-create-index-harness-failure.json` — failed first harness attempt
  caused by an incorrect payload-index endpoint;
- `qdrant-long-path-baseline.json` — Windows path-length observation from a prior
  diagnostic root;
- `qdrant-server.log` — redacted server log corresponding to the diagnostic runs.

Any future qualification must rerun the accepted qualification packet against the
exact selected executable, client, configuration and product revision and record a
separate immutable receipt.
