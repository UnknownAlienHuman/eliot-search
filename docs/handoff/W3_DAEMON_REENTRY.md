# W3 daemon re-entry boundary

`eliot-searchd` re-enters at W3 only after the accepted W2 daemon package/API handoff and accepted `W2_G1` receipt. Its W1/W2 packets are replaced, not replayed.

The W3 daemon writer reads only:

```text
new materialized W3 daemon context
accepted prior eliot-searchd W2 API/handoff
accepted W2_G1 receipt
exact accepted W3 library handoffs
W3 Qdrant artifact/client/schema/qualification receipts
eliot-searchd assignment and FUNCTIONS.md
W3_IMPLEMENTATION_PACKET.md
```

The write scope remains `bins/eliot-searchd/**`. No prior implementation source is included. W3 composition may add only the exact qualified lexical/index profile, supervised Qdrant process/data adapters, publication route and pin/reclaim services. Query-product, current-workspace, proof, lifecycle, client-edge, evaluation and optional-depth profiles remain absent.

Readiness requires coherent build/configuration/handler/route/profile identities and accepted qualification evidence. Qdrant process health, a responding endpoint, collection presence or alias name alone cannot establish ownership, current visibility or indexed readiness.
