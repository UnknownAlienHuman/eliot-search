# eliot-search-model-worker

**Optional isolated P16 model worker.**

**Status:** the dependency-free binary is an explicit refusal stub. Its existing unavailable-invocation fixtures remain; no model runtime/profile is selected or qualified. The unused model-provider dependency closure was removed under #528.

Hosts one exact qualified model profile over private daemon-only IPC with finite resource, cancellation,
content-minimization, crash-isolation and removal semantics. It never opens Search stores/index or an
external client endpoint.

- **Default:** binary absent or stopped.
- **Gate:** exact accepted P15 + ADR + G6 candidate qualification.
- **Soft source target:** 4,500 lines.
- **Functions:** [FUNCTIONS.md](FUNCTIONS.md)
- **Agent instructions:** [AGENTS.md](AGENTS.md)
