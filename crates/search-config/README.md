# search-config

Replace the legacy parser/registry/receipt stack with small typed settings consumed by real owners. The typed
replacement is a target, not a working configuration interface or a reason to expose dormant settings.

[FUNCTIONS.md](FUNCTIONS.md) owns capture, precedence, paths, validation, redaction and status. Start with
[Foundation fields](FUNCTIONS.md#foundation-fields-shipped-with-root-work); later fields arrive with consumers.
Use the [layer rules](FUNCTIONS.md#layer-semantics), [path rules](FUNCTIONS.md#target-capture) and
[future verification cases](FUNCTIONS.md#focused-checks-after-code), not historical configuration catalogues.

Coordinate [#333](https://github.com/UnknownAlienHuman/eliot-search/issues/333) with actual daemon callers.
No runtime readiness follows from accepting a config file. Remove this package if the remaining typed merge
fits more simply in daemon composition.
