# Windows secret boundary

The unused Credential Manager/DPAPI Rust package, manifest and exclusive fixtures are removed.
The plain local search product does not need a credential vault, DEK selection or protection adapter.

Preserve existing user credentials, keys and files. Do not execute the retired credential cleanup route.
Actual native file identity and owned-child process cleanup remain with their live source/client owners.
