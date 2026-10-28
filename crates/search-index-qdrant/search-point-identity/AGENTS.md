# Working on point identity

Read [FUNCTIONS.md](FUNCTIONS.md) and the current [delivery stage](../../../docs/implementation/README.md#delivery-sequence).
Keep deterministic bounded encoding, the full identity distinct from its address, and collision refusal.

Implement only the selected stage. Complete generations need no cross-generation continuity, occurrence matching,
incarnations or validity intervals. Add that conditional machinery only when Stage 3A measurements require 3B.

Replace the old revision-bound S11/FNV API with its actual projection/publication/bridge/query callers. Use a fresh
physical generation, not dual minting or migration. If the remaining helper is small, inline it and delete the
package. Run minimal pinned check/Clippy for actual consumers after code; old golden bytes are not product gates.
