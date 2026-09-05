# Unlanded CI-generated patches

Baseline inspected: `d80ed9def94e75793c3078e40bd90cfd6a7818db` (September 4, 2026, America/New_York).

The following workflows were removed from the active workflow directory because they violated the manual-only policy and attempted to generate and commit application code from CI:

- `.github/workflows/zz-one-shot-daemon-control-redb.yml`, original blob `a3b75984ca27f1de546825c963b1942305ff27b1`.
- `.github/workflows/zz-one-shot-redb-encoded-mutation.yml`, original blob `09cadb4ec1498d1f79fb0f779b88652388e546fb`.

Their full contents remain in Git history at the baseline commit. They are proposals, not landed implementations. Run `33938159506` failed at `Require clean qualified baseline` on both platforms; the implementation and verification steps never ran. Do not count these workflows or their commit titles as evidence of working persistent redb integration.

The retained workspace-check workflow is manual-only, read-only, checks the exact dispatched SHA and performs one `cargo check`. It neither rewrites source nor pushes commits. A passing check is not runtime, security or Qdrant qualification.
