# Historical W8 provider hardening

The previous W8 programme treated pairing challenges, durable bindings, grant issuance, capability
descriptors, recipe admission, handle reauthorization and revocation drain as mandatory protocol mechanics.
That system is not the baseline local product and is not the connected serving path.

Trusted child stdin/stdout or an owner-only local OS pipe uses one bounded typed frame and direct daemon
scope resolution. It does not need an application pairing challenge, per-frame proof, durable binding record,
standalone grant or capability digest.

A selected restricted-client adapter may authenticate a real external client and own its allowed source/view
subset, expiry and revocation. It terminates into the same current dispatcher. Revocation cancels that
session and drops its windows; it does not require protocol-owned durable handle/continuation invalidation or
a recipe registry.

Remove W8 pairing/binding/grant/descriptor/recipe/handle APIs and their crash/replay matrices with their
callers. Keep only the direct framed-session behavior in [README.md](README.md) and
[FUNCTIONS.md](FUNCTIONS.md): finite prefix/body/output limits, current typed request/result DTOs, correlation,
cancellation, original deadline, one terminal event and fail-stop partial output.
