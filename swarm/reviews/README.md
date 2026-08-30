# Independent package reviews

Canonical layout:

```text
swarm/reviews/<package>/<review_id>.toml
```

A review checks one exact submission against its ticket/context/lease chain, primary contract, stage
obligations, dependency handoffs, complete diff, raw evidence, ownership boundaries, public-surface
digests and line budget. The reviewer identity must differ from the writer and bind the exact submission
commit and digest.

An accepted review permits the integration owner to publish a package handoff. It does not itself accept
a gate, wave, Product Pulse report or optional-depth candidate. Package writers cannot accept their own
work.

Use `swarm/REVIEW_RECEIPT_TEMPLATE.md` and `swarm/schemas/independent-review-v1.toml`. This directory
currently contains no accepted review.
