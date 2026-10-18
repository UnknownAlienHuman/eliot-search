# search-index-reclaimer — removal target

Current source contains pure caller-shaped manifest/plan builders and a synchronous exact-delete executor.
There is no production `execute_batch` caller or durable checkpoint owner. Public proofs, sampled pin
watermarks, custom plan digests and checkpoints would create a second publication journal.

Keep the useful effect only:

- exact retired-ID deletion/readback on the same route/generation; or
- whole old-collection deletion after a physical-generation switch.

Publication already owns the old/new views, exact retired work and pending artifact. After no new old-view
guard can be acquired and the last existing guard drops, it advances a bounded cleanup cursor. Uncertain
delete keeps the same operation pending and reads actual state.

Move the private effect helper to publication/bridge and remove this package with its public plan/proof/
receipt APIs and fixtures. Ordinary cleanup does not delete retained source bytes, implement legal purge or
claim physical erasure.

[FUNCTIONS.md](FUNCTIONS.md) · [View guards](../search-epoch-pins/FUNCTIONS.md) ·
[Implementation](../../../docs/implementation/README.md)
