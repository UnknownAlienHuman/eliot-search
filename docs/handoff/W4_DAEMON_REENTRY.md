# W4 daemon re-entry boundary

`eliot-searchd` re-enters at W4 only after the accepted W3 daemon package/API handoff and accepted `W3` receipt. Its W1–W3 contexts are replaced, not replayed.

The W4 daemon writer reads only:

```text
new materialized W4 daemon context
accepted prior eliot-searchd W3 API/handoff
accepted W3 receipt
exact accepted W4 library handoffs
accepted W4 query qualification receipts
eliot-searchd assignment and FUNCTIONS.md
W4_IMPLEMENTATION_PACKET.md
```

The write scope remains `bins/eliot-searchd/**`. No prior implementation source is included. W4 composition may add only the exact authenticated baseline query handlers, access checkpoints, bounded plan/execution/validation pipeline, compact results, handles, continuations and evaluation seams. Current-workspace, proof, lifecycle, client-edge, Product Pulse and optional-depth profiles remain absent.

Readiness requires coherent config/handler/route/recipe identities and accepted dependency/qualification evidence. Qdrant health, candidate retrieval or source-handle possession cannot grant access, prove currentness, validate source evidence or establish complete coverage.
