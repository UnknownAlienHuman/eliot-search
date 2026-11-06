# Source-backed subject selection

The unused supplied-candidate resolver Rust package, manifest and exclusive fixtures are removed.
No subject-resolution handler is currently connected.

Add this operation only with a real source-backed consumer. Resolve explicit source/revision/range
through the pinned inventory and retained reader; missing explicit locations are not silently replaced.
A cursor needs actual containing mapped ranges. Otherwise ordinary bounded retrieval may nominate
alternatives while preserving names, signatures, repository/document distinctions and ambiguity.
A mention or matching name does not establish identity; truncated candidates cannot prove uniqueness.

Keep any needed ordering beside that consumer. No mandatory ladder, equivalence receipt, context
fingerprint or independent revalidation platform.
