# Local query wire

One parent-owned daemon child accepts one Query (request ID 1) and correlated Cancel frames over stdio.
A frame is a four-byte little-endian JSON body length followed by bounded Serde JSON. The shared 8 MiB
ceiling includes the prefix. Stdout contains one terminal; diagnostics use stderr.

See [FUNCTIONS.md](FUNCTIONS.md) for the actual DTOs, whole-item output bound and validation.
There is no authentication, pairing, grant, MAC, replay or endpoint protocol.
