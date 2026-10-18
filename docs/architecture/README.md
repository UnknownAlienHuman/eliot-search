# ELIOT Search — architecture

ELIOT Search helps agents and people understand large codebases and document collections without
reading the entire corpus. It finds useful evidence, shows where it came from and makes the next
source read or exact check obvious.

This is the product manifesto to keep in the lead developer's context. The
[implementation guide](../implementation/README.md) owns code locations, integration and donor choices.
This manifesto replaces conflicting requirements in the historical 8.4 design and old task packets.

## Product

A user selects local repositories or corpora, indexes them and asks a question. Search returns relevant
files, symbols or document ranges with short excerpts, revision identity and clear gaps. The user can
open the exact evidence, orient themselves in an unfamiliar project and compare implementations.

The application works on its own through a CLI and local client interface. Large repositories and
portfolios are a core workload. A model, agent controller or ELIOT integration is not required to run it.

## What a useful result means

- A citation corresponds to the exact source revision that supplied its bytes.
- A ranked result helps navigate; it does not claim exhaustive coverage or prove absence.
- An explicit exact check can establish a complete result over a stated, actually scanned scope.
- Changes become searchable incrementally. Restart preserves committed state; a lost index can be rebuilt.
- Results expose unsupported formats, missing revisions, stale observations and unfinished work clearly.
- Interactive requests remain bounded and cancellable while background indexing makes progress.

## Design principles

**Source truth has one owner.** Retained source bytes and their coordinates establish evidence.
Qdrant is the only indexed retrieval engine. Control storage tracks current application state.
Search does not maintain another search database or a parallel authority for the same fact.

**One coherent view.** A query uses one explicit source scope and one published view. Incomplete writes
never become a mixed visible index. Recovery resolves real unfinished effects. Ordinary reads and pure
transformations need no durable receipts.

**Local by default.** The baseline serves the owning OS account and explicitly selected sources.
It neither executes indexed material nor silently broadens the corpus. Restricted clients, remote
adapters and encrypted storage are separate needs with real consumers, not universal prerequisites.

**Complexity must buy useful behavior.** Prefer a direct call and a concrete data structure. An
abstraction earns its place by removing duplication or isolating a real dependency or effect.
A capability name, agent assignment or hypothetical future backend does not justify another subsystem.

**Libraries supply established machinery.** Reuse parsing, matching, serialization, storage and
transport implementations. Search adds source semantics and useful results. It does not need its own
compiler, database transaction platform or general cryptographic protocol.

**Hashes have a specific purpose.** Content identity and necessary integrity checks use real hashes.
A local request, plan, configuration, state transition or review does not need a digest or signature
merely to look trustworthy. A trusted local exchange needs no separate cryptographic identity system.

**Replace obsolete designs.** Internal and pre-release formats carry no backward-compatibility promise.
Legacy implementations and their fixtures do not constrain the new product. Original files and sole
retained copies remain user data; simplification never silently destroys them.

**Cost follows useful work.** Normal updates touch changed content. Storage and maintenance do not grow
with an eternal history of completed internal operations. Query bookkeeping is short-lived. Optional
features do not multiply the normal startup and query paths.

**Completion means an application.** Deliver connected behavior and remove the replaced implementation.
Neither small files, a low line count, closed tickets nor passing structural checks establish a working
product. Tests should expose incorrect results, lost state or unbounded work.

## Product boundary

Search owns source acquisition, retained revisions, preparation, indexed retrieval, exact verification
and compact evidence for its clients. Agent scheduling, task governance, general memory, online research,
a hosted service and universal data migration are outside this application.

Optional code intelligence, document formats, semantic ranking and adapters extend the same source and
query path when selected. Their future possibility is not a reason to build an empty framework now.
