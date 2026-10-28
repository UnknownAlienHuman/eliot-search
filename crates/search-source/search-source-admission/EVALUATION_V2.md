# Historical admission evaluation V2

This is a historical implementation pointer, not the current contract or a compatibility requirement.
The [full R055 description](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/crates/search-source/search-source-admission/EVALUATION_V2.md) preserves the old API, limits and fixtures.

The old evaluator is pure policy logic over supplied observations. Retain useful fail-closed pre-read
selection and post-read narrowing over actual acquired bytes, not decision fingerprints, canonical preimages
or a permanently frozen V2 API. Supported empty input follows the current source/preparation contract.

Follow [FUNCTIONS.md](FUNCTIONS.md) for the current ingestion sequence and removal boundary. Trace the actual
caller before changing Rust; move useful behavior and delete displaced APIs/fixtures together. This documentation
change does not delete the existing implementation or establish its runtime correctness. Preserve user data;
unsupported old roots are refused before writes rather than migrated through the V2 archive machinery.
