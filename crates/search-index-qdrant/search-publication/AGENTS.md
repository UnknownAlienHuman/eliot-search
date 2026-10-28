# Working on publication

Read [FUNCTIONS.md](FUNCTIONS.md), then only the affected bridge/control/view/query consumers.

Stage 2/3A uses one serialized publisher, fixed complete generation input, InsertOnly build, exact readiness,
bounded evidence/current commit and explicit view installation. No sequence, closure or occurrence matching.
Conditional Stage 3B follows its own recipe only after failed measured budgets.

Cleanup is whole-generation in both profiles; no physical point deletion or point-cleanup ledger. Never treat a
timeout or matching readback as proof that every earlier dispatched writer has stopped. No redb writer/view lock
spans backend or object I/O. Local object collection remains stopped-root maintenance.

Change actual consumers and remove displaced machines/fixtures together. Run minimal locked Rust 1.98
check/Clippy after production changes. Algorithms and failure rules stay in the recipe, not duplicated here.
