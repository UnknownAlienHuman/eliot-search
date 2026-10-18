# Index cleanup — source and consolidation

Ordinary cleanup belongs to the publication/view lifetime, not a separate proof-and-journal platform.
Keep exact Qdrant delete/readback mechanics; fold their orchestration into the publisher and remove this
package when callers switch.

## Current source findings

| Current source | Problem |
|---|---|
| [`validate_retired_manifest`](src/manifest.rs) | Compares fields from two caller-supplied structures. It does not derive IDs from committed publication or bind the digest to `point_ids`. |
| [`plan`](src/plan.rs) | Copies the whole manifest into batches and uses a custom FNV-like digest/short operation IDs. |
| [`execute_batch`](src/execute.rs) | Useful exact delete/readback skeleton, but the port lacks route/generation/deadline binding and cleanup is detached from current pins. |
| public plan/checkpoint/receipt types | Allow caller-constructed progress and require another durable journal. |

A sampled `ReclamationWatermark` cannot close the acquisition race. Use the current-view guard model in
[`search-epoch-pins`](../search-epoch-pins/FUNCTIONS.md): after cutover no new old-view guard can exist,
so the last guard drop is the deletion boundary.

## Cleanup owned by publication

The committed view transition already knows the old/new manifests, route/generation and exact retired IDs.
Store cleanup progress in the same bounded publication artifact:

```text
cleanup kind: exact retired IDs | whole retired generation
route / generation / retirement epoch
ordered ID pages when needed
next page/effect/readback cursor
last unresolved effect, if any
```

Do not create a second operation identity, manifest proof, receipt graph or permanent checkpoint history.
The existing pending operation identity and artifact pages identify the work.

### Same physical generation

After every view that can observe the retired interval has drained:

1. read the next bounded exact ID page;
2. call the bridge delete on the same route/generation with the remaining deadline;
3. read back exact absence of those IDs;
4. advance the publication cleanup cursor;
5. remove completed page data when no view needs it.

A timeout/disconnect after dispatch is unresolved. Read actual state and continue; never infer rollback or
blindly use a new operation ID. A partial or foreign response blocks progress. No broad filter deletion.

### Replaced generation

An incompatible vector/profile/schema normally uses a new physical generation. Old views keep the old
collection. After the current pointer changed and its last guard is gone, delete the old collection as one
bounded external cleanup operation. Do not manufacture per-point retire records for an entire obsolete
generation.

## Artifact and source GC

Roots are intentionally small:

- current published view and its manifest;
- pending publication artifact;
- live view guards/query windows;
- current source heads;
- explicitly retained user data that the product actually supports.

View manifest entries lead to their exact source revisions, prepared artifacts and retrieval points.
When no root refers to an immutable object, the owning store can delete it by exact object reference.
Unknown inventory/readback leaves the object in place. Ordinary source removal publishes a view without
that source; physical cleanup follows guards and references. This is not security purge and makes no
secure-erasure claim.

Tantivy's segment updater keeps files listed by committed and in-flight metadata and lets its directory GC
remove untracked files. Lucene snapshot deletion keeps generations while references exist. Reuse these
small rules, not either backend.

## Removal boundary

After publication and bridge callers use this flow, remove:

- `PublicationCommitProof` and field-equality validation;
- public `ReclaimPlan`, batch/checkpoint/receipt constructors and custom digests;
- the separate reclaimer operation journal/ports;
- route/epoch watermark inputs;
- generic retention/purge dependencies and their fixtures.

Keep only private helper code that performs exact bounded ID deletion/readback if it remains useful inside
publication or the bridge.

## Required behavior checks

- old query continues while new view is current;
- no new old-view guard after switch;
- exact point cleanup starts only after the last relevant guard;
- whole-generation cleanup waits for all old-view guards;
- timeout after delete resolves by same-route/generation readback;
- partial/unexpected IDs block cursor advancement;
- current/pending/pinned manifest and source artifacts survive GC;
- unreferenced derived artifacts eventually disappear without an eternal history.
