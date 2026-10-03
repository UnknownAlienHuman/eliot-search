# Archived ticket-issuance qualification

> **Inactive historical fixture.** Ticket issuance, writer leases, acknowledgements, context
> materialization and launch-state advancement are not ELIOT Search product capabilities and are not
> prerequisites for implementation. ADR 0005 and ADR 0006 supersede this directory as active
> qualification scope.

The files retained here describe an earlier repository-local orchestration experiment. They do not
materialize context, issue authority, qualify Search or provide required build/release evidence.

Do not:

- run these fixtures as a prerequisite for Search code;
- port their schemas/validators into new Rust controller tooling;
- treat a result as product readiness, package acceptance or permission to implement;
- add actor credentials, signing profiles, tickets or lease records to Search.

Required Search qualification lives in product-specific directories such as:

```text
qualification/qdrant/
qualification/query/
qualification/current/
qualification/proof/
qualification/lifecycle/
qualification/client-edge/
qualification/product-pulse/
qualification/optional-depth/
```

Product tooling cleanup under issue #214 will remove or archive controller-only commands, workflows and
fixtures after preserving any genuinely product-relevant validator in a focused owner.

Historical bytes remain recoverable from Git. Reusable orchestration work belongs in
`UnknownAlienHuman/eliot-swarm-controller` and later `eliot-memory-os`.
