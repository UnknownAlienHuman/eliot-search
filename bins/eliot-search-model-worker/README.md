# eliot-search-model-worker

**Optional isolated P16 model worker.**

**Status:** complete worker contract; binary/runtime/model are unselected and implementation is blocked.

Hosts one exact qualified model profile over private daemon-only IPC with finite resource, cancellation,
content-minimization, crash-isolation and removal semantics. It never opens Search stores/index or an
external client endpoint.

- **Default:** binary absent or stopped.
- **Gate:** exact accepted P15 + ADR + G6 candidate qualification.
- **Soft source target:** 4,500 lines.
- **Functions:** [FUNCTIONS.md](FUNCTIONS.md)
- **Agent instructions:** [AGENTS.md](AGENTS.md)
