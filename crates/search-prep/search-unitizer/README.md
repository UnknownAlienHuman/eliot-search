# search-unitizer

[prepare_utf8](src/prepared.rs) composes the exact UTF-8 materializer, reused line-preferred range
mechanics and one complete bounded artifact encoding. [decode_prepared_utf8](src/prepared/decode.rs)
accepts saved representation bytes and returns borrowed text plus bounded ordered UnitSpan descriptors.
[FUNCTIONS.md](FUNCTIONS.md) owns binding, execution and saved-artifact acceptance.

<a id="accepted-v3-baseline"></a>
The active artifact contains actual text, not a v3 UnitSet or a layout-only manifest. Its ordinal is
position within this immutable artifact, not cross-revision continuity or a Qdrant point ID.

<a id="markdown-structural-v4"></a>
<a id="jats-structural-v5"></a>
Old format envelopes and exclusive fixtures are removed. A future selected format must connect its
actual producer and evidence consumer instead of restoring dormant scaffolding.