# Windows local pipe transport — optional persistent profile

This small package owns one concrete Windows client pipe handle and synchronous byte I/O. It is not the
baseline CLI path and owns no protocol, pairing, grant, endpoint discovery or access policy.

The first working local product uses owned child stdin/stdout under #280/#332. Keep this package only if a
persistent owner-only Windows pipe is selected after that path works.

## If retained

The platform owner, not this package's pipe name alone, must establish that the connected server belongs to
the expected OS user/installation. This package then:

- opens one explicitly supplied canonical pipe endpoint once;
- retains one non-clone owned handle;
- checks byte-pipe/local shape available from the OS;
- performs finite partial reads/writes under the caller's original absolute deadline/cancellation loop;
- maps EOF/would-block/content-free OS failures;
- closes only its handle.

Connection success is not authentication, source authority, a grant, a peer acknowledgement or rollback
proof. A selected restricted-client adapter authenticates separately and terminates into the same current
framed dispatcher.

## Remove if not selected

Delete the package, native registration/credential constructors and pipe-listener scaffolding if child stdio
remains the shipped local route. Do not keep it as a mandatory second transport, compatibility facade or
future plugin seam.

Do not add:

- pairing/MAC/grant logic;
- endpoint files/token files/TCP fallback;
- arbitrary handle adoption;
- listener/process lifecycle;
- background threads, reconnect or pooling;
- durable connection receipts.

## Verification

Only for a selected persistent-pipe profile: actual same-owner connection, wrong-user/foreign-server refusal
in the platform owner, byte mode, partial I/O, would-block, deadline/cancel, EOF, backpressure and handle
cleanup through the current framed protocol. Source presence and pure fixtures are not installed behavior.
