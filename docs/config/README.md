# Configuration

Source-removal observations below describe the refactor branch. Main retains legacy configuration with its
consumers until a separate source cut; these files are not current replacement settings.

The typed replacement is selected, not yet connected. Add fields with the owners that actually consume them.
Do not implement a registry of future settings or present legacy examples as a working configuration.

| Question | Owning section |
|---|---|
| Which fields arrive first? | [Foundation fields](../../crates/search-config/FUNCTIONS.md#foundation-fields-shipped-with-root-work) |
| Capture invocation/file/environment once | [Target capture](../../crates/search-config/FUNCTIONS.md#target-capture) |
| Precedence, omission, lists and validation | [Layer semantics](../../crates/search-config/FUNCTIONS.md#layer-semantics) |
| Native and relative paths | [Target capture](../../crates/search-config/FUNCTIONS.md#target-capture) |
| Redaction and useful errors | [Diagnostics and secrets](../../crates/search-config/FUNCTIONS.md#diagnostics-and-secrets) |
| Status, restart and live changes | [Status and runtime changes](../../crates/search-config/FUNCTIONS.md#status-and-runtime-changes) |
| Dependencies | [Donor limits](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md#typed-configuration-donor-boundaries) |

#333 tracks the cutover. A runnable example ships with the real typed decoder and consumers.
[Implementation owners](../implementation/README.md#owners) define field meaning; the settings decoder does not
own publication, storage or query policy. Historical wave/section catalogues stay in Git history.
