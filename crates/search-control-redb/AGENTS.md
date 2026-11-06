# Plain root control storage

Use [FUNCTIONS.md](FUNCTIONS.md). This crate currently owns only the plain-root format marker and the three
operations in `plain_control`; keep the public module path and bounded error semantics stable with consumers.

Initialization and semantic-empty probing require the caller's root exclusion and owned unfinished-init marker.
Ordinary format inspection must stay read-only, without catalogue enumeration or a whole-file size ceiling.
Read-only empty proofs are bounded before enumeration. A preformat retry may write only the exact proven empty
file after its reader closes, must abort redb repair, and must not overwrite an existing application format.

Keep the original deadline/cancellation inputs and distinguish unavailable I/O from corrupt or unsupported data.
Preserve unsupported original files. Do not restore legacy journals, migration/authority/DEK frameworks, test-only
compatibility producers, source bodies, another search store, or dependencies for removed APIs.

Source-only checks do not establish native handle behavior, cancellation recovery or runtime qualification.
