# Working on the daemon

The daemon owns bootstrap/root operations, retention, preparation, publication and one framed read-only search.
Preserve the owned child, original deadline and root/view through terminal flush; join the synchronous backend
worker outside the I/O runtime. [FUNCTIONS.md](FUNCTIONS.md) names the actual consumers.

Publication/recovery verifies the full saved graph, backend readback and independent checked canary before
acceptance. Persist only compact references; reuse one verified Prepared buffer/decoded view across adjacent
same-source pages per pass. Publication returns state/counts, not user hits.

Search pins bounded committed metadata, then checks only returned evidence and retained objects using decoder
expectations from that same snapshot. Never add a per-query corpus scan, latest-preparation substitution, live
fallback, repair/reprepare or write. Keep whole-unit framing and explicit missing/corrupt failure scope.
Only the bridge uses SDK types; the operator owns Qdrant. Preserve unresolved target obligations and root safety.
