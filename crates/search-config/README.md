# search-config

This crate still implements the legacy generic configuration platform. Startup/status, DIRECT root-locator
hints and the explicit DEK-selection command have distinct consumers; most registered values do not reach
their operational handlers. A valid non-default query limit is still receipt-gated at startup.

Read [FUNCTIONS.md](FUNCTIONS.md) for the source/caller map and the selected replacement. It is the single
home for decoding, merge, path, validation, startup, status and runtime-change rules.

## Next change

[#333](https://github.com/UnknownAlienHuman/eliot-search/issues/333) replaces the generic parser/tree,
registry, fingerprints, action plans and accepted-readiness receipts with concrete typed settings. Change the
real daemon and setting consumers together. Startup constructs owners from validated values; status reads
running settings and actual component observations.

High-level `toml` plus Serde is selected, not installed. The current custom decoder uses
`toml_parser 1.1.4+spec-1.1.0`; pin replacement dependencies/features with consuming code and regenerate the
lock. The [layer recipe](FUNCTIONS.md#sparse-layers-and-validation) defines omissions and precedence, and the
[path policy](FUNCTIONS.md#paths-and-diagnostics) distinguishes Unicode TOML from OS-valued overrides.

Retain the real locator and protected storage consumers until their owners move. Coordinate actual #598
object/key overlap without inheriting its old generic security programme. If the remaining typed helpers fit
in daemon composition, remove this crate rather than preserve a facade.

The unused repository config catalogues and duplicate section packets have been removed. The
[configuration index](../../docs/config/README.md) links current recipes and immutable historical evidence;
no packet, registry row or parser-compatible example defines working product configuration.

## Verification

After code changes, use the [focused behavior cases](FUNCTIONS.md#focused-checks) through actual consumers
and the pinned compiler/Clippy for changed targets. This documentation does not establish a running typed
settings path or a passing current binary build.
