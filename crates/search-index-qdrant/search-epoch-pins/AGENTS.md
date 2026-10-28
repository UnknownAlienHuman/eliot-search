# Working on view lifetime

Read [FUNCTIONS.md](FUNCTIONS.md) and the
[publication lifetime rule](../search-publication/FUNCTIONS.md#view-lifetime-and-whole-generation-cleanup).
Extract one private current-view manager: strong current Arc, weak retired references and owned guards.

Pin only current; a window inherits an existing guard. No public route/epoch acquisition, renewal or watermark
API. After durable current commit, open/verify the matching snapshot and install it under a short manager lock.
No manager lock spans redb writer, object or Qdrant I/O. Last guard drop only wakes cleanup.

Whole-generation cleanup waits for every live view/window and targetable unresolved attempt. Do not introduce
point deletion based on sequence or immediate-predecessor drain; neither baseline nor Stage 3B uses a physical
point reclaimer. Restart reconstructs current; ordinary interactive windows expire.

Move surviving callers and delete old registry/continuation/receipt fixtures. Run minimal Rust 1.98 check/Clippy
for the affected production consumers after code.
