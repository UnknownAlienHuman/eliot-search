# Working on the daemon

The daemon owns native invocation/bootstrap, four root operations, retain-file and retained-only prepare-source. Keep the existing root child, root guard and original execution deadline. Follow [FUNCTIONS.md](FUNCTIONS.md): object I/O and stored-artifact decoding precede the short source/preparation CAS; output follows commit.

Prepare-source preserves selected settings and emits complete stored text/ranges with the JSON-derived stdout bound. Never reopen the live source or rebuild a missing/corrupt Ready artifact. Do not restore legacy admission, receipts, replay, Git, inventory, query, Qdrant or temporary transport routes. Preserve root/control safety. Runtime qualification belongs to the designated verifier.
