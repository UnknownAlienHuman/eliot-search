# Working on the daemon

The daemon owns native invocation/bootstrap, the four root operations, and the first retain-file operation. Keep the existing root child and root guard path. For retain-file, capture current source state, acquire one bounded file, publish and read back its plain ObjectRef, commit one guarded current row, and emit the result only after commit.

Do not restore legacy admission, receipt, registry replay, Git, inventory, query, Qdrant, or temporary transport routes. Preserve the existing root/control safety behavior. Runtime qualification is owned by the designated verifier.
