# Working on search-continuation

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and
[query cutover](../README.md#one-query-window).

This package is a removal target. Do not harden its token, durable-replan, security-fence, issued-set or
cleanup-effect model. Move only the useful O(page) cursor/output-commit behavior from the daemon paging
code into one application `QueryWindow`.

Requirements for the surviving owner:

- pin exactly the published view used by the query;
- preserve final rank and advance one cursor after complete output only;
- read/verify only selected-page ranges under one absolute deadline;
- retain issued item locators only until the original bounded expiry;
- delete terminal/expired state and release the view guard;
- expire all windows on restart.

Do not add durable query history, replay suppression sets, pin permits, cryptographic token authority,
remote acknowledgement protocols or compatibility wrappers. Change actual result contracts and callers
in the same cutover, then remove this package.
