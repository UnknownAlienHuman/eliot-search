# Working on search-ports

This catalogue is a deletion target, not the location for new abstractions. Read the affected replacement in
[FUNCTIONS.md](FUNCTIONS.md), then inspect its real current callers and receiving owner.

Use the root [implementation choices](../../docs/implementation/WORKING_GUIDE.md#implementation-choices).
Standard library traits and a narrow local interface may solve a real boundary; they do not require multiple
production backends, a conformance registry or a ports-v2 package. Do not add dummy implementations to justify one.

Move one connected consumer path to its concrete owner. Carry only needed execution state, preserve actual
failure semantics, and remove displaced imports/adapters/exclusive fixtures together. No ordered rewrite of all
port families precedes Foundation. Run minimal locked check/Clippy for affected owners and immediate consumers.
