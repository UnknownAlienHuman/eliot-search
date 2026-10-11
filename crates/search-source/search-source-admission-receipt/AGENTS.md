# Agent contract — search-source-admission-receipt

Own only this package's pure v2 receipt issuance, current-policy verification,
private immutable receipt/verified values, canonical receipt rows and causal
fixtures. Source-admission owns policy compilation, portable observation models
and effective decisions; shared contracts owns encoding and algorithm execution.

Keep the one-way dependency on `search-source-admission` and `search-contracts`.
No public supplied receipt/digest constructor, v1 fallback, forwarding legacy
module, second evaluator/codec/crypto implementation, native/root/access/currentness
authority, source opening, clock/deadline renewal, durable state or vendor types.
Caller checkpoints retain their original Cancelled/DeadlineExceeded meanings.

Preserve the receipt@2 profile/domain and exact accepted bytes. API import-path
migration is explicit; future source adoption belongs to #270/#271. Any new
load-bearing profile change needs separate review. Do not restore the deleted
old module or invent a cross-package reexport/dependency cycle.

Whole tracked handwritten Rust, including inline/external fixtures, has a source
review ceiling of 1,200 lines for the #424 relocation. This is a bounded initial
owner, not blanket future growth permission. Split along real responsibilities.

Use root execution protocol: exact Ready/freeze, sole manager/writer/source PR,
production-lib locked check/strict Clippy and affected static guards, exact-head
review, fixtures authored with code and execution deferred to integrated #215.
Source compilation does not qualify native/runtime/installed product behavior.
