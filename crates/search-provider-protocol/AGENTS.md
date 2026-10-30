# Working on the local wire

[FUNCTIONS.md](FUNCTIONS.md) owns frames, control, clocks and terminal races. [Local scope](../../docs/architecture/LOCAL_SCOPE.md)
selects owned-child stdio for the same OS account. No authentication, restricted clients, grants, pairing, MACs,
replay history or second transport is selected.

Keep finite input/output, partial-frame state, cancellation service, typed correlation and complete-terminal
validation. Never insert Cancel bytes into a partial frame. Prepare window updates before output and commit only
after complete local flush; that is not peer acknowledgement. Change DTO/codec/client/dispatcher/CLI callers together.
Minimal locked compiler/Clippy follows code; tests follow the connected product. Do not add a protocol framework.
