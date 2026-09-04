# search-os-secrets-windows

Concrete Windows current-user DPAPI effect adapter.

It protects only caller-supplied short secrets, requires canonical optional
entropy bound to user, installation, incarnation, and purpose, rejects UI
fallback, redacts plaintext in `Debug`, and overwrites decrypted owned buffers
on drop.

The crate owns no persistence. `search-os-secrets` remains the pure lifecycle
authority; daemon and control-store composition must persist only
`ProtectedSecret` bytes and exact lifecycle receipts.

On non-Windows targets every cryptographic operation returns
`DPAPI_UNSUPPORTED_PLATFORM`; validation and type checking remain available.
