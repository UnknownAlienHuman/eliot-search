# Query family

**Organizational capability family — not a Cargo package.**

## Child packages

- [`search-access/`](search-access/) — C18: Validate grants, intersect scope with authoritative state and compile noninterfering pre-candidate access/scoring legs.
- [`search-overlay/`](search-overlay/) — C19: Represent current saved and authenticated unsaved deltas as bounded direct candidates and shadows.
- [`search-exact/`](search-exact/) — C20: Compile and execute bounded exact scans against a frozen authoritative denominator and produce truthful proof reports.
- [`search-subject-resolver/`](search-subject-resolver/) — C21: Resolve an entity under an explicit source view using a deterministic ladder and return ambiguity instead of guessing.
- [`search-query-planner/`](search-query-planner/) — C22: Compile a normalized recipe, coherent view, validated grant and budgets into an immutable vendor-neutral SearchTaskPlan.
- [`search-retrieval-executor/`](search-retrieval-executor/) — C23: Execute direct, exact, indexed and optional-provider legs under bounded queues, cancellation and deterministic fusion.
- [`search-candidate-validator/`](search-candidate-validator/) — C24: Convert nominated candidates into validated source-backed evidence candidates or explicit stale/gap reasons.
- [`search-comparator/`](search-comparator/) — C25: Align validated implementations by lineage, evidence role and behavior observations without declaring a normative winner.
- [`search-result-projector/`](search-result-projector/) — C26: Project validated candidates, comparison and exact reports into bounded evidence-oriented responses and handles.
- [`search-continuation/`](search-continuation/) — C27: Own bounded opaque continuation state without exposing vendor cursors or pinning snapshots indefinitely.

## Family invariants

- Access and currentness are pre-candidate constraints, not final filters.
- Query plans are server-owned and vendor-neutral.
- Candidate validation reopens exact source revisions before projection.
- Results are bounded candidate products, never belief or admission decisions.

Each writer agent owns exactly one child package and follows that package's `AGENTS.md`.
