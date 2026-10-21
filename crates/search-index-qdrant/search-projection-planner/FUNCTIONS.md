# Projection: changed-source diff, not a whole-corpus plan

At `c8e38b6`, `plan_projection` and `PublicationAdmissionOwners` feed package/process fixtures rather than a
connected daemon publisher. `PointSpec` discards actual vectors in favour of digests. The builder creates all
points, clones manifests, constructs canonical trees and re-encodes them; its advertised 100,000-point limit
conflicts with the 4,096-item codec ceiling. `manifest_read.rs` decodes and re-encodes the entire object.
Those costs are not fixed by merely renaming the output 'pages'.

## Concrete input and ordering

Work on changed sources, not a globally materialized UnitSet. Preparation supplies one bounded source product,
exact revision/prepared refs and actual retrieval inputs. The unchanged-source check skips preparation when
both the retained bytes and relevant preparation/retrieval settings are unchanged.

The [unitizer map](../../search-prep/search-unitizer/FUNCTIONS.md) owns occurrence matching. After matching,
sort that source's finite point descriptors by point ID in place. Read its old ordered bindings with a
source-prefix range in the [control tables](../../search-control-redb/FUNCTIONS.md). Merge those two sequences
into create/retain/close operations and evidence changes. Do not assume preparation emits hash-derived IDs
already sorted, or sort the complete corpus to make the iterator interface look streaming.

For m new units and o old units in the changed source, sorting is O(m log m) and the merge is O(m + o), apart
from database access. Memory includes the bounded source product, its descriptors and one output batch; it
is not constant in source size. State/enforce the source and metadata-batch limits before allocations/effects.
Large repositories are many independently updated sources, not one giant atomic input.

## Occurrence versus point versus citation

An occurrence is reused only under the conservative matching rule in the unitizer map. Retrieval identity
includes its incarnation, exact retrieval input/profile/role and selected stable source/group meaning.
Revision and absolute citation coordinates are absent. A changed input gets a new retrieval point even if the
occurrence is the same; changed coordinates only update evidence metadata. Duplicate text is not identity.

Allocate unmatched occurrences from the operation's target sequence and source-local new-unit slot, scoped
by source identity. This is an internal birth value, not a continuity rule based on ordinal. Persist it with
the prepared publication input; recovery uses that same input. Once external work starts, never allocate a
replacement birth or reopen a closed point interval. No eternal occurrence registry is needed.

## Output required by the real consumer

Immutable operation pages contain actual BM25 Document text/options or selected vector values, full/narrow
point operations, validity closures and the compact evidence delta. A digest cannot stand in for uploadable
input. Header/page bounds are checked on write and independent load; no second canonical-value tree or
receipt establishes correctness.

The query lookup layout is not left to the implementer: compact source/evidence rows and pinned redb snapshots
are specified in the control map. There is no custom page-directory/index or corpus-sized manifest cache.
Operation pages are temporary recovery inputs, not a second permanent evidence database.

## Publication unit

An incremental operation covers complete changed sources within the configured metadata-commit ceiling.
Validate the final evidence/source delta before Qdrant work. Split between sources; never publish half of
one source as if it were complete. Oversize/unsupported input is an explicit gap with the previous published
revision still identifiable. A new-generation rebuild fills staging metadata in bounded transactions and
switches its pointer only when complete; see the [publisher](../search-publication/FUNCTIONS.md).

## Remove

Remove whole `ProjectionPlan`/manifest copies, two full diff maps, digest-only vector output, revision-bound
S11/scoring IDs, artificial allocation arithmetic and test-only admission-owner proof APIs. Keep useful
validation, exact point identity, source-local sorting and merge logic. A small remainder belongs directly
in preparation/publication, not a new planner service.

## Cases and measurements

Check: edit at the start of a file; repeated equal blocks; insert/remove/reorder a duplicate; same input under
changed heading context; disappear/reappear after committed removal; a source exceeding the ceiling; and a
rebuild while source heads advance. Measure actual retained/new/closed points, changed metadata rows,
preparation bytes and Qdrant writes. Cache hits alone do not establish incrementality.
