# eliot-search

The CLI captures native arguments/settings, uses one owned local client session and renders typed source-backed
results. It does not open redb/object/Qdrant state or own scope, planning or another process manager.

Read [FUNCTIONS.md](FUNCTIONS.md) for invocation, modes, presentation, cleanup and removal boundaries;
[AGENTS.md](AGENTS.md) for editing constraints. Load protocol/client recipes only when those interfaces change.

The connected retained-source → Qdrant → excerpt path remains implementation work under #264/#332.
[Evidence status](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) distinguishes inspected source from executed
commands. Command names, help output and compiled dependencies alone do not establish a working CLI.
