# search-projection-planner

**C13 — canonical Qdrant projection and manifest producer.**

This package converts admitted typed unit/vector inputs into exact point specifications without performing vendor I/O. It is the only producer of the closed S9.5 point payload and the immutable S11.3 projection manifest.

## Owns

- exact projection profile-set/vector validation;
- S11 point-key input construction;
- closed S9.5 payload construction;
- canonical payload/vector digests;
- exact point specs and point-ID-ordered manifests;
- create/retain/retire diff by exact UUID.

## Must not own

- source acquisition or policy/access decisions;
- Qdrant transport or collection names;
- broad-filter closure when exact manifest IDs exist;
- source membership, ACL, path or text disclosure in Qdrant payload;
- mutation/publication visibility.

Source membership and expected payload/vector digests remain in the immutable manifest/control plane. Qdrant stores only the opaque S9.5 payload and rebuildable vectors.

- **Delivery wave:** W3 / P06
- **Agent instructions:** [AGENTS.md](AGENTS.md)
