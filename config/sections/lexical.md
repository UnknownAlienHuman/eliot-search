# `[lexical]` configuration contract

- **Owner:** `search-lexical`
- **Earliest wave:** W3
- **Section minimum:** `NEW_COLLECTION_GENERATION`
- **Secret policy:** `forbid_plaintext`
- **Canonical source:** `docs/config/CONFIGURATION_1.0.md`

This packet is the bounded read set for the section owner. It does not authorize implementation or
override `swarm/launch-state.toml`.

## Fields

| Key | Type | Default / bound | Override | Minimum action |
|---|---|---|---|---|
| `profile_id` | `ProfileId` | `accepted qualified profile` | `file` | `NEW_COLLECTION_GENERATION` |
| `code_profile_enabled` | `bool` | `true only in CODE profile` | `file` | `NEW_COLLECTION_GENERATION` |
| `neutral_text_profile_enabled` | `bool` | `true in LEXICAL/CODE profiles` | `file` | `NEW_COLLECTION_GENERATION` |
| `collision_threshold_profile` | `ProfileId` | `accepted collision fixture ID` | `file` | `NEW_COLLECTION_GENERATION` |

## Required section API

```text
section_descriptor() -> ConfigSectionDescriptor
compiled_defaults() -> ConfigSectionInput
validate_section(input, platform, accepted_capabilities) -> Result<ValidatedSection, ConfigError>
section_digest(validated) -> Blake3Digest32
plan_section_change(old, new) -> Result<SectionReloadDecision, ConfigError>
apply_live_change(old, new, context) -> Result<ConfigApplyReceipt, ConfigError>
```

`apply_live_change` is legal only for fields whose final action is `APPLY_LIVE`. Restart, security
barrier, rebuild, collection-generation and gate actions are executed by daemon orchestration.

## Invariants

- tokenizer/hash/weighting/BM25 semantics are one immutable profile
- provider path never switches automatically
- profile change cannot reinterpret points in place

## Required tests

- defaults validate and produce a stable section digest;
- every field boundary and override source is tested;
- unknown keys and wrong types fail closed;
- the owner never accepts plaintext secrets or foreign section keys;
- a field cannot be assigned a weaker action than this packet;
- redacted diagnostics contain no secret, source/query content or raw path;
- equal validated inputs produce equal digests and reload decisions.
