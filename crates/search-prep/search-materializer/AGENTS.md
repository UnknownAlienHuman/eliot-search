# Working on search-materializer

<a id="keep-the-useful-work"></a>
Read [FUNCTIONS.md](FUNCTIONS.md) and only the affected unitizer/object/source-row boundary. Implement the selected
format, not a preparation framework or all parsers at once.

<a id="reduce-copies-and-duplicate-owners"></a>
Keep the verified owned input through decoding and unitization. Preserve actual coordinates/loss and input/output
limits; preparation uses ExecutionContext, not a query view. Saved artifacts follow the recipe's acceptance rule.
Do not add a second writer, adjacent replay, proof token or accepted-profile registry.

<a id="internal-formats-and-verification"></a>
Change producer and real consumers together. Reuse existing maintained parser/range mechanics when that format
is selected; source content stays inert. Preserve original bytes and unpublished work, not obsolete crypto APIs.
After code run minimal locked check/Clippy for affected production consumers. Later cases remain in the recipe.
