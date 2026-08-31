# W2 daemon re-entry boundary

`eliot-searchd` re-enters at W2 only after the accepted W1 daemon package/API handoff and the accepted
W1 receipt. Its W1 packet is replaced, not replayed. The W2 writer reads the new materialized W2 context,
the accepted prior daemon handoff, the W1 receipt, exact accepted W2 library handoffs, the daemon
assignment/`FUNCTIONS.md` and `W2_IMPLEMENTATION_PACKET.md`.

The W2 write scope remains `bins/eliot-searchd/**`. No W1 implementation source or W1 packet is included.
The goal is DIRECT source-spine composition only; Qdrant/index/query/future-wave profiles remain absent.
