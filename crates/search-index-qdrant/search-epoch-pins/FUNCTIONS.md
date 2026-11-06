# Published-view lifetime

The unused epoch-pin registry Rust package, continuation adapters, manifest and exclusive fixtures
are removed. The actual publication/query owner retains the physical generation and matching redb snapshot.

A request owns its view through work and terminal output. Later same-session windows inherit that view.
No arbitrary epoch/route acquisition, renewal, sampled watermark or second pin catalogue.
Whole-generation cleanup waits for actual live references and unresolved targetable effects.
No shared mutable view lock spans object/backend/output work; dropping a guard performs no deletion.
Local-object collection is a separate stopped-root operation.
