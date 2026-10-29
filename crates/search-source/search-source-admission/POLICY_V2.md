# Historical admission policy V2

This is a historical implementation pointer, not the current contract or a compatibility requirement.
The [full R055 description](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/crates/search-source/search-source-admission/POLICY_V2.md) preserves the old API, limits and fixtures.

`policy_v2::compile_policy_v2` compiles bounded rules and the existing `globset` matcher.
Keep useful matching and pre-read denial, not canonical policy fingerprints or accepted-limit identities.

Follow [FUNCTIONS.md](FUNCTIONS.md) for the current ingestion sequence and removal boundary. Trace the actual
caller before changing Rust; move useful behavior and delete displaced APIs/fixtures together. This documentation
change does not delete the existing implementation or establish its runtime correctness. Preserve user data;
unsupported old roots are refused before writes rather than migrated through the V2 archive machinery.
