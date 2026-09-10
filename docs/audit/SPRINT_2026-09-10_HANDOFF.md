# Sprint handoff — 2026-09-10

Baseline at close: `b28e36b`. Measured, not asserted; every number below has the command
that produced it.

## State of `main` at close

```
cargo check-all                                            -> exit 0
cargo test -p eliot-searchd --all-targets --locked --no-fail-fast
                                                           -> 178 passed / 4 failed
cargo clippy -p search-continuation  ... -D warnings        -> 46 findings
cargo clippy -p search-publication   ... -D warnings        -> 34 findings
every other workspace package                               -> 0 findings
```

At sprint open the workspace **did not compile**: `cargo check-all` exited 101 on the freshly
fetched `main`. It has been green since PR #143 and stayed green through 37 merges.

## What landed

37 pull requests, #143 through #182. Thirty-five were clippy cleanups taking packages to zero
findings under `-D warnings`; one was a genuine defect fix (#147, below); one was repository
hygiene (#182, 20 stray patch transcripts removed from the root).

Two of them are worth reading as engineering rather than lint work:

- **#147** — 26 tests in `search-control-redb` could never have passed on the pinned
  `x86_64-pc-windows-msvc` target. redb holds an exclusive byte-range lock for the life of the
  database, and the tests called `std::fs::read` on the same path while it was open. Advisory on
  Linux, enforced on Windows. Fixed by retaining a `File::try_clone` duplicate of the handle
  given to redb and reading through it — a duplicated handle shares the lock ownership, so the
  bytes are sampled at exactly the same points with no unlock, drop or reopen. 219 tests pass
  parallel and single-threaded.
- **#176** — 738 of 769 findings in `bins/eliot-searchd`, including 30 `needless_pass_by_value`
  signature changes with all call sites updated in the same commit.

## What remains — clippy

Two packages, both mechanical. Counts and families measured on `b28e36b`.

### `crates/search-query/search-continuation` — 46 findings

```
36  clippy::missing_const_for_fn
 4  clippy::missing_panics_doc
 2  clippy::struct_excessive_bools
 2  clippy::needless_pass_by_value
 2  clippy::unused_self
```

Doc: master **S26 "Handles and continuations"** (line 1569).

Warning on `missing_const_for_fn`: adding `const` to a function that calls non-const code
fails with `E0015`. Such a function is not eligible — leave it and say so. This exact mistake
broke the build once during the sprint.

### `crates/search-index-qdrant/search-publication` — 34 findings

```
12  clippy::doc_markdown
10  clippy::too_long_first_doc_paragraph
 4  clippy::redundant_pub_crate
 2  clippy::suspicious_operation_groupings
 2  clippy::collapsible_if
 2  clippy::too_many_lines
```

Doc: master **S13 "Publication model"** (line 879). Two thirds of these are documentation-only.

## Open defects — not lint work

### 1. Four `direct_store` test failures, one of them a real Windows lock bug

```
direct_store::revision_writer::tests::corrupt_protected_object_is_not_replaced_by_plaintext_reindexing
direct_store::revision_writer::tests::process_exit_after_ciphertext_before_catalog_is_recoverable_without_plaintext
direct_store::revision_writer::tests::referenced_plaintext_migration_keeps_the_original_revision_identity
direct_store::storage_io::tests::empty_encoded_object_is_rejected_before_creating_a_file
```

Three fail with OS error 33, `ERROR_LOCK_VIOLATION` — the same class #147 fixed for
`search-control-redb`. The test helper walks the data root and `fs::read`s every file,
including `.eliot-search-owner.lock`, which the fixture's own owner guard holds region-locked.

Closed PR #144 had the correct fix and is worth re-applying onto current `main`: skip the owner
lock file in the walk, and add a `count_objects` helper that distinguishes revision ciphertext
from deterministic preparation objects, since both share the `.dpapi` extension on Windows and a
bare extension count cannot tell orphan reuse from duplication. That branch was closed only
because both files it touched were rewritten by #176 and #178, making the rebase costlier than
re-applying. Expect 178/4 before and 182/0 after.

### 2. The tests leak a Windows credential per revision key and never clean up

`bins/eliot-searchd` tests write `ELIOT Search/revision-key/<hash>` entries into the Windows
Credential Manager and never remove them. Several hundred accumulate. Once the store fills,
`CredWrite` starts failing and unrelated tests fail with `DIRECT_REVISION_KEY_WRITE_FAILED:8`
or `DIRECT_REVISION_KEY_MISSING` — including tests that have nothing to do with credentials.

During this sprint 320 leaked entries were purged once and 27 more accumulated afterwards.

Fix: delete each key the test created, in a guard that runs on both the pass and panic paths.
Prove it by showing the credential count unchanged across a full test run:

```powershell
$before = (cmdkey /list | Select-String 'ELIOT Search/revision-key/').Count
cargo test -p eliot-searchd --all-targets --locked --no-fail-fast
$after  = (cmdkey /list | Select-String 'ELIOT Search/revision-key/').Count
```

`$before` must equal `$after`. To clear an accumulated store, delete via Win32 `CredDelete`
filtered strictly on the `ELIOT Search/revision-key/` prefix — `cmdkey /delete` silently fails
on target names containing a space.

## Two measurement traps that produced wrong conclusions during this sprint

Both of these cost real time and led to incorrect reports. Record them.

1. **`cargo test` truncates at the first failing binary.** Because `src/entry.rs` fails on the
   four pre-existing `direct_store` tests, a plain `cargo test -p eliot-searchd --all-targets`
   stops after 2 of 15 targets and silently hides the rest. It reported 91 passed where the real
   figure was 178. **Always pass `--no-fail-fast` in this package.**
2. **Never count clippy findings from text output.** `grep -c '^error'` also counts
   `could not compile` lines — it reported 423 where the truth was 6 — and a plain run aborts at
   the first failing crate, hiding every package behind it. The only honest count is

   ```
   cargo clippy -p <pkg> --all-targets --locked --message-format=json -- -D warnings
   ```

   counting JSON objects where `message.level == "error"` and some span has `is_primary: true`.

A related structural effect worth expecting: clearing a package's findings regularly *raises*
the workspace total, because clippy can then reach packages it previously aborted before.
The count went 206 -> 145 -> 109 -> 769-in-one-package as walls came down. That is progress,
not regression.

## What this sprint did NOT touch

This matters more than anything above. Thirty-five of the thirty-seven merged PRs were lint
hygiene. **No delivery gate moved.** The product gaps named in `README.md` and in
`docs/audit/ELIOT_SEARCH_AUDIT_2026-09-04.md` are exactly where they were:

- **The Qdrant data plane does not exist.** `crates/search-index-qdrant/search-qdrant-bridge/src/lib.rs`
  still opens by stating it "does not discover or start Qdrant" and is an in-memory model for a
  future concrete adapter. There is no Qdrant client dependency. Gate **G2** cannot be claimed.
- **No gate has recorded evidence.** S37 defines G0 through G6 (line 2117); `qualification/`
  holds no run that closes any of them. `qualification/CURRENT_FAILURES.txt` still describes
  commit `78c70d0` from 2026-09-05 and should be refreshed or removed.
- **Release qualification, native Windows security, and the canonical
  source/residency/preparation contracts** are untouched.

Two corrections to the 2026-09-04 audit, confirmed by reading the code this sprint:

- `search-control-redb` **does** depend on real redb (`redb = "=2.6.3"`). The claim that it is an
  in-memory reference model with no redb dependency is stale.
- The workspace contains **zero** `todo!()` or `unimplemented!()`, and exactly one crate
  documents itself as a model — the Qdrant bridge.

## Suggested order for the next run

1. Finish the two clippy packages above. Small, mechanical, unblocks a green `cargo clippy-all`.
2. Fix the credential leak. It corrupts every subsequent test run and will keep producing
   failures that look like code defects and are not.
3. Re-apply the #144 owner-lock fix. Gets `eliot-searchd` to 182/0.
4. Refresh `qualification/CURRENT_FAILURES.txt` against a real measured run at HEAD.
5. Only then start on a gate. **G1** (DIRECT without Qdrant) is the cheapest real one, because
   the pieces already exist and what is missing is scripted end-to-end evidence on a clean data
   root plus the same run after a restart.

`cargo clippy-all` going green is worth having, but it proves toolchain hygiene, not that the
product works. Nothing in this sprint moved the project closer to a qualified release; it made
the codebase reviewable enough that the work which does can proceed on a green baseline.
