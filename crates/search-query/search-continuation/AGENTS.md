# Working on query windows

[FUNCTIONS.md](FUNCTIONS.md) owns the window algorithm; read it and the actual protocol/evidence/daemon callers.
The local product has no restricted-client revocation or security-generation system.

Keep one session-owned window with its pinned view and final EvidenceRefs. Emitted `(window_id, item_index)`
locators are not the internal evidence vector. Reserve state before output; commit progress only after complete
transport output. No mutable window/map borrow or mutex spans reads, encoding or writes. Cancellation/expiry
remain serviced while the operation owns its pending state. Do not renew expiry or reuse IDs.

Move useful slicing and prepared-output behavior with real callers; remove both old catalogues, token minting,
source-fence scans, durable replan/receipt histories and exclusive fixtures. Keep retained-range verification,
spent-work accounting and bounded emitted-item expansion. Minimal locked check/Clippy follows code; behavioral
tests follow the connected product. No forwarding window service or second transaction protocol is needed.
