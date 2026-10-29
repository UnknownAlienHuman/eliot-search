# Historical admission observations V2

This is a historical implementation pointer, not the current contract or a compatibility requirement.
The [full R055 description](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/crates/search-source/search-source-admission/OBSERVATION_V2.md) preserves the old API, limits and fixtures.

The old observation/locator records classify supplied metadata; they do not prove native containment,
stable acquisition or source currentness. Replace their fingerprint model with the current small before/after
read facts supplied by the native source owner.

Follow [FUNCTIONS.md](FUNCTIONS.md) for the current ingestion sequence and removal boundary. Trace the actual
caller before changing Rust; move useful behavior and delete displaced APIs/fixtures together. This documentation
change does not delete the existing implementation or establish its runtime correctness. Preserve user data;
unsupported old roots are refused before writes rather than migrated through the V2 archive machinery.
