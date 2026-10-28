# Working on the dormant model worker

Read [FUNCTIONS.md](FUNCTIONS.md), the
[model design note](../../crates/search-model-provider/README.md) and the
[current delivery sequence](../../docs/implementation/README.md#delivery-sequence).

## Currentness boundary

R033 inspected a 1,047-byte dependency-free refusal stub and found no production daemon caller at the inspected
head. That finding is revision-specific. Before deleting the package, repeat an exact current-branch search of
workspace/manifests, Rust imports/constructors/path inclusion, features, tests, workflows and non-Rust tooling.

If the stub and no-caller result remain true, delete the package, workspace/lock references and exclusive
unavailable fixtures. If a real caller now exists, identify it and retain only the smallest truthful unavailable
boundary while that caller moves. Do not harden or qualify the refusal path.

Baseline Search uses Qdrant native BM25. A model path returns only after the connected lexical product exists and
a measured exact model/profile has a direct preparation, query or reranking consumer.

When that happens, select the exact artifact, tokenizer, task/role, options, packaging, license and finite
resource policy with the consumer. Prefer a bounded in-process library when operationally acceptable. Create a
worker only when a real native dependency, crash boundary or resource-isolation requirement justifies a child.

Never create a generic provider registry, plugin framework, qualification receipt chain, model lifecycle OS,
query-rewriting/answer agent or another transport protocol. A selected worker accepts bounded encode/rerank
input and returns validated vectors or an order/subset of supplied candidates. It cannot discover sources,
widen scope, write Qdrant independently, create evidence or generate answers.

After future code, run minimal Rust 1.98 compiler/Clippy for the concrete model owner and its real consumers,
then measure the candidate against the lexical baseline. Until a concrete consumer is selected, deletion—not
refusal-stub hardening—is the selected direction.
