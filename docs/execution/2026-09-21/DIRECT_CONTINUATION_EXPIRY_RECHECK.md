# DIRECT continuation: reject expiration during page preparation

Date: 2026-09-21. Base: `3a697cd7f9dc57128f36f78bdeb3a49d1fe49899`.
Tracking: T21 / PR #118; follows the separate whole-window-copy correction.

## Defect and intentional behavior change

The outer continuation operation checked TTL before computing the current source
fence. Page copying, token construction and expiry cleanup happened afterwards.
A deadline crossed during this work could therefore return an already-expired
page, even after the sweep had removed its token. A zero remaining TTL did not
prevent returning candidates; an exhausted final page omitted TTL entirely.

The paging operation now checks the original monotonic deadline twice: before
copying any matches, and after page construction/cursor handling/expiry cleanup,
immediately before returning the page. Expiry at either checkpoint returns the
existing `DIRECT_CONTINUATION_EXPIRED` error, drops the whole affected window and
returns no page, candidates or continuation token. This also applies to a
would-be final page. Cleanup remains idempotent after exhaustion or expiry sweep.

For a still-live nonterminal page, remaining TTL is computed from the final
checkpoint, never from a renewed or reconstructed deadline. Page-size, entropy,
session, live-barrier and exact namespace/source-fence validation remain in their
previous order. The private clock seam does not provide an external bypass:
production delegates to `Instant::now`; only local fault tests supply samples.
The preceding no-whole-window-clone change remains intact.

This is a catalog-return checkpoint, not an atomic socket-disclosure guarantee.
Transport cancellation, final live authorization and output-time checks remain
separate integration obligations. No new deadline policy, error code, token
format, public API, dependency, persisted state or authority record was added.

## Deterministic regression source

Five added tests, without sleeps, files or credentials, cover:

- expiration before materialization;
- expiration during preparation, for both intermediate and final pages;
- remaining TTL measured at the final checkpoint without deadline renewal;
- unknown-token refusal before clock observation;
- late expiry plus the ordinary sweep preserving an unrelated live window and
  exact retained-match accounting.

The injected instants and records are synthetic test inputs, not execution or
security-qualification receipts. All eight earlier paging tests and every
pre-existing continuation test remain in place.

## Verification

The baseline catalog blob is `8e5337ba56c203e9275eb960ca99484969b68044`.
Scoped `git diff --check`, preservation of the original public validation path
and first-page/lifecycle code, absence of full-record clones, and exact local
Git blob identities were checked. The local repository is scope-only.

Compilation, Rust unit/process tests, rustfmt and Clippy: **NOT_RUN**. Cargo is
absent in this execution environment; the recorded command attempt exits 127.
Required pinned-toolchain commands remain those in
`DIRECT_CONTINUATION_WINDOW_ADVANCEMENT.md`. No Actions run, independent review,
canonical T21 acceptance or measured performance claim is issued.
