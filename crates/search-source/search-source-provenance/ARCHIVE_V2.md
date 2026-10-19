# Historical original-input archives v2

The v2 policy/admission archives preserve complete normalized policy rules, observations, decisions and
receipt fingerprints for pre-release registry/source-record formats. They are not part of the current-format
application.

The archive/fragments can exceed one control value, require canonical maps/arrays, whole-archive digests,
part references and complete reconstitution through policy compiler/evaluator/receipt issuer. None of that
proves a stable file read or retained source. It exists to reproduce the old operation history.

Current ingestion stores:

```text
policy revision
closed final outcome/reason
selected classification
current RevisionRef / immutable ObjectRef
```

It does not store complete policy rules, original locator/metadata arrays, observation/decision/receipt
fingerprints or archive fragments. Restart reads current rows directly.

The old `eliot.source-original-policy@2`, `eliot.source-original-admission@2` and
`eliot.original-input-fragments@2` bytes are historical only. Unsupported roots depending on them are
refused before writes and left intact; they are not migrated, rebuilt or reclassified as current authority.

Delete the archive codecs, fragment layout/storage proof, source-record v4 references, registry-control v4
restore paths and fixtures with this package after #270 moves current ingestion. See [README.md](README.md).
