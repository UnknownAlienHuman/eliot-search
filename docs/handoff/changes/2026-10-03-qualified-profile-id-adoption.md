# Qualified profile identifier syntax decision

**State: ACCEPTED_INTEGRATION_CONTRACT — syntax only.** Independent Luna Max review of
`7da866e2c60f91453f3ca813904e5127e6f2456e` accepted the grammar component separately from the
bootstrap proposal, which still needs correction and review. This decision establishes no profile
registration, namespace ownership, qualification, trust pin, actor assignment or issuance authority.

`QualifiedOpaqueId` is one exact ASCII string `namespace/local_opaque_id`. There is exactly one literal
forward slash. The namespace is 1–96 bytes and matches `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`.
The local component is 1–128 bytes and retains the existing case-sensitive `OpaqueId` grammar
`^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`. The complete value is 3–225 bytes. Reject extra separators,
empty components, Unicode, whitespace, URL encoding, escapes, controls and normalization. Preserve
the exact accepted bytes and case; parsing never repairs an input.

The combined pattern is
`^[a-z][a-z0-9]*(?:-[a-z0-9]+)*/[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`.
The separate namespace limit remains mandatory: the total-byte cap and combined pattern alone do
not enforce it. `QualifiedProfileRefV1` is a semantic name for this syntax, not an additional
registered type or a version selector embedded in the value.

Apply this grammar to the existing `ImmutableArtifactRef.store_profile_ref_is_qualified_opaque_id`
and `ImmutableSignatureRef.approval_profile_ref_is_qualified_opaque_id` rules without changing their
record field order or weakening `OpaqueId`. A later registry edit adds exactly one named type after
the four accepted ticket enums, increasing the type count from 51 to 52 while retaining type-registry
format 2 and record schema 1. That registry edit and its conformance are pending at this decision.

An integration-only pure parser may now implement this accepted grammar with finite input bounds.
It must identify its scope as `NON_AUTHORITATIVE`, reject unknown syntax and preserve exact borrowed
components. Valid syntax does not resolve a profile, prove that its namespace belongs to this
project, or establish qualified store/signature behavior. Those checks require separately accepted
registries, actual provisioning, current trust inputs and executed independent qualification.

The external-authority channel, local host pin, bootstrap evidence, activation and actor mappings
remain separate proposed contracts. No package implementation or control-record mutation is
authorized by this syntax correction.
