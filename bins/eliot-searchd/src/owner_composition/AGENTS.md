# Native root owner composition boundary

This directory owns the concrete native root/installation/owner adapter. Root policy remains in
search-runtime-owner. It grants no source catalog, control-store, secret, preparation, query or
Qdrant child authority. Normal caller cutover/deletion remains #355; explicit initialization and
named recovery have separate serial children under #354.

For the first native admission/inspection slice only, a reviewed size exception covers this exact
owner_composition family (including owner_composition.rs and every retained/new private fixture)
with a 3,500 whole Rust-line ceiling. The only additional adapter is native_root_command.rs and its
entry.rs wiring. The change remains <=2,500 production additions and <=30 production paths. This is
a concrete native exclusion/identity boundary, not a daemon-wide size allowance or new package.
Existing daemon-wide size/deletion debt remains unresolved under its existing cleanup owners.
A successor cannot reuse this exception without reviewing its own concrete closure.

Keep the legacy installation/owner transcripts and all old fixtures. The new existing-only native
path accepts the separately identified format-2 native layout, refuses legacy format-1 authority,
and performs no initialization or repair. Both actual locked handles and the retained root handle
must bind the pinned layout before capability admission and throughout revalidation.

One original absolute deadline and cancellation flag survive through all work and terminal output.
Charge every filesystem/ABI-observer seam call, open and requested read attempt cumulatively; the
initial meter includes the request's single qualified entropy call. The bound describes owner I/O
seam invocations, not every internal Win32 call, CPU/RSS or hard preemption. Short reads/writes are
charged and polled individually. Every post-effect failure remains unknown/fenced; Drop never writes.

Production Rust1.98 locked/offline check and strict Clippy plus affected static guards are source
gates. Unchanged daemon D387 diagnostics stay NOT_PASS and are compared exactly. Private native
fixtures remain authored NOT_COMPILED/NOT_RUN until integrated qualification under #215.