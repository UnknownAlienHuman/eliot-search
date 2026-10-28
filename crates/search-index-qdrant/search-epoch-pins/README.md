# Published-view lifetime

Keep the exact physical collection and evidence snapshot used by a request/window alive. Replace the inherited
raw pin registry with a small private current-view manager; no arbitrary old-view acquisition or renewal API.

[FUNCTIONS.md](FUNCTIONS.md) owns acquisition and the source cutover.
[Publication cleanup](../search-publication/FUNCTIONS.md#view-lifetime-and-whole-generation-cleanup) owns deletion
eligibility and uncertainty. Last guard drop only wakes it.

Both Stage 2/3A and conditional Stage 3B use whole-generation cleanup. There is no physical point deletion based
on sequence, predecessor drain or sampled watermarks. All live guards and targetable unresolved attempts matter.

Trace real callers, move the small manager into publication/query if appropriate, and remove the old pin,
continuation-binding and receipt APIs with exclusive fixtures. Historical source details remain in the
[pre-R055 README](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/crates/search-index-qdrant/search-epoch-pins/README.md).
