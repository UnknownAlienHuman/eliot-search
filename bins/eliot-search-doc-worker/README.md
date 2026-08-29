# eliot-search-doc-worker

**Optional isolated P17 document materializer worker.**

**Status:** complete worker contract; provider/runtime/artifact are unselected and implementation is
blocked.

Hosts one exact no-execute/no-network document profile over private daemon-only IPC. Enforces bounded
container/page/object/image/decompression/output resources, malformed-input isolation, exact input
identity, coordinate/loss maps, assurance and cleanup/removal.

- **Default:** binary absent or stopped.
- **Gate:** exact accepted P15 + provider ADR + G6 candidate qualification.
- **Soft source target:** 5,000 lines.
- **Functions:** [FUNCTIONS.md](FUNCTIONS.md)
- **Agent instructions:** [AGENTS.md](AGENTS.md)
