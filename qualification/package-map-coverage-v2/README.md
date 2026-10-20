# Historical package-map coverage qualification

This directory qualified the generated Architecture 8.4 relationship/package-map system. That policy is
retired under [#214](https://github.com/UnknownAlienHuman/eliot-search/issues/214).

Do not run or preserve:

```text
xtask generate/validate coverage-graph
xtask generate/validate package-maps
xtask validate architecture-coverage*
validate-function-packets
```

The suite proved only that checked-in registries, Markdown headings, declared modules, package bundles and
Cargo edges agreed with one another. It did not establish implementation correctness, and it now blocks
package deletion/merging by treating the old topology as authority.

Delete this qualification directory, its workflow, generated outputs, wrappers and exclusive fixtures with
the retired commands. Historical reports remain in Git. Current boundaries are checked by Cargo metadata,
compiler privacy/direct API review and the connected application's behavior.
