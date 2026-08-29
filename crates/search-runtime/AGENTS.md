# Family rules

- Own lifecycle, retention and purge mechanics; never retrieval meaning.
- The daemon is the composition root; this directory itself is not a Cargo package.
- Security/legal purge dominates ordinary retention.

This directory is not a package. Do not add a family-level `Cargo.toml` or shared implementation. Put behavior in the child package that owns the failure state and test seam.
