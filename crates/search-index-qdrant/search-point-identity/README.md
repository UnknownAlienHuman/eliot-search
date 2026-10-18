# search-point-identity

The stateless `s11` module supplies the canonical point-identity API from #256. Planner and publication
already use it; #262 still ports the bridge. #329 retires the separate root legacy exports and registry
from the actual caller census. Neither source presence nor a completed API port establishes indexed
product qualification.

[S11_PROFILE.md](S11_PROFILE.md) is the precise key/schema/domain/budget, UUID, collision and migration
contract. This package owns that preimage and full-identity comparison over the shared canonical/hash
owner; it does not own another codec, source identity, registry or upsert engine.
A 128-bit UUID address retains all bits and is not sufficient overwrite evidence.

[AGENTS.md](AGENTS.md) records package boundaries and fixture disposition.
