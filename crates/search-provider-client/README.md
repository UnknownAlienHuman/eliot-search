# Owned-child query client

QueryClient retains one parent Tokio runtime. query_owned_child starts one explicit daemon executable with
native arguments, sends one framed SearchRequest, services stdout/stderr/control concurrently, validates the
whole terminal and observes/reaps the child before returning QueryCompletion.

Diagnostics are retained to 64 KiB while excess bytes are still drained. The CLI owns safe rendering and its
process-level cancellation flag. See [FUNCTIONS.md](FUNCTIONS.md). This crate has no endpoint discovery,
credentials, reconnect, replay, named-pipe adapter or public session framework.
