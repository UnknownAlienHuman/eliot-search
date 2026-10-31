# eliot-search

R068 implements one native argument capture and direct dispatch in the installed `entry.rs → app` path.
Local help/version share one implementation; forwarding borrows the captured arguments. Five unlinked copies
are removed. Existing DIRECT handlers remain: typed settings, the serviced client and safe typed rendering are
still target behavior, not completed by this invocation change. The CLI must not own source/store/query state.

Read [FUNCTIONS.md](FUNCTIONS.md) for invocation, modes, presentation, cleanup and removal boundaries;
[AGENTS.md](AGENTS.md) for editing constraints. Load protocol/client recipes only when those interfaces change.

The connected retained-source → Qdrant → excerpt path remains implementation work under #264/#332.
[Evidence status](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) distinguishes inspected source from executed
commands. Command names, help output and compiled dependencies alone do not establish a working CLI.
