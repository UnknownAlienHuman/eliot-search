# search-continuation

Replace the shared and daemon continuation/handle stores with one process-local window in the real
query/session owner. The [FUNCTIONS map](FUNCTIONS.md) is the single page, lifetime and output-commit recipe.
Other files in this package are migration/source notes, not additional state-machine requirements.

A live session keeps final order, cursor, fixed expiry and its pinned view. One-shot output returns no
locators which would expire when its child exits. Continue/Expand get their own request deadline capped by
window expiry. Page progress counts handled references represented by cards/gaps, not the proposed slice or
just the number of cards. Partial output commits nothing and fails the session.

Move direct cursor slicing, actual retained-range checks and prepared output commit to the query owner,
then delete this package and duplicate catalogues. No durable replan, token proof, source fence or terminal
history is required. This describes the target, not a delivered implementation.
