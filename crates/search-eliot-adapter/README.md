# search-eliot-adapter

**C30 optional ELIOT profile — Optional ELIOT adapter.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Map ELIOT external-provider contracts to generic Search contracts as a disabled-by-default leaf package.

## Owns

- WorkScope/disclosure to grant mapping
- SourceView/StateFence mapping
- capability pulse projection
- Search result to ELIOT provider-result translation
- binding/session mapping

## Must not own

- ELIOT canonical DB credentials or writes
- memory/admission/finish dispositions
- Qdrant/redb types
- importing ELIOT internals into Search core crates
- creating a new authority surface

- **Delivery wave:** W8 / optional P14 profile
- **Soft source-line target:** 5,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
