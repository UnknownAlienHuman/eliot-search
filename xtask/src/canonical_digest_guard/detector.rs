use std::collections::BTreeMap;

use syn::visit::{self, Visit};
use syn::{Expr, Item, Lit};

use super::syntax::{crypto_import, crypto_name, digest_name, expr_name, name, test_attrs};

pub(super) struct Detector {
    pub(super) findings: BTreeMap<(String, String, bool), usize>,
    scope: Vec<String>,
    test: bool,
    depth: usize,
    pub(super) error: Option<String>,
}

impl Detector {
    pub(super) const fn new(test: bool) -> Self {
        Self {
            findings: BTreeMap::new(),
            scope: Vec::new(),
            test,
            depth: 0,
            error: None,
        }
    }

    fn add(&mut self, signal: &str) {
        if self.error.is_some() {
            return;
        }
        if self.findings.len() >= 16_384 {
            self.error = Some("finding budget exceeded".into());
            return;
        }
        let symbol = if self.scope.is_empty() {
            "<file>".into()
        } else {
            self.scope.join("::")
        };
        *self
            .findings
            .entry((symbol, signal.into(), self.test))
            .or_default() += 1;
    }

    fn attrs(&mut self, attrs: &[syn::Attribute]) {
        for attr in attrs {
            if !matches!(
                name(attr.path()).as_str(),
                "cfg"
                    | "cfg_attr"
                    | "derive"
                    | "doc"
                    | "test"
                    | "must_use"
                    | "allow"
                    | "expect"
                    | "deny"
                    | "forbid"
                    | "inline"
                    | "repr"
                    | "non_exhaustive"
                    | "deprecated"
                    | "default"
                    | "path"
                    | "ignore"
                    | "should_panic"
                    | "track_caller"
                    | "cold"
                    | "export_name"
                    | "no_mangle"
                    | "link"
                    | "link_name"
                    | "serde"
            ) {
                self.add("attribute-macro-review");
            }
        }
    }
}

impl<'ast> Visit<'ast> for Detector {
    fn visit_item(&mut self, item: &'ast Item) {
        if self.error.is_some() {
            return;
        }
        let (symbol, attrs): (String, &[syn::Attribute]) = match item {
            Item::Fn(f) => (f.sig.ident.to_string(), &f.attrs),
            Item::Mod(m) => (m.ident.to_string(), &m.attrs),
            Item::Impl(i) => (format!("impl[{}]", type_name(&i.self_ty)), &i.attrs),
            Item::Const(c) => (c.ident.to_string(), &c.attrs),
            Item::Static(s) => (s.ident.to_string(), &s.attrs),
            Item::Struct(s) => (s.ident.to_string(), &s.attrs),
            Item::Enum(e) => (e.ident.to_string(), &e.attrs),
            Item::Trait(t) => (t.ident.to_string(), &t.attrs),
            Item::Type(t) => (t.ident.to_string(), &t.attrs),
            Item::Use(u) => (format!("use[{}]", use_name(&u.tree)), &u.attrs),
            Item::Macro(m) => (
                format!(
                    "macro[{}]",
                    m.ident
                        .as_ref()
                        .map_or_else(|| name(&m.mac.path), ToString::to_string)
                ),
                &m.attrs,
            ),
            _ => ("<item>".into(), &[]),
        };
        let was_test = self.test;
        self.test |= test_attrs(attrs);
        self.scope.push(symbol);
        self.attrs(attrs);
        if let Item::Impl(implementation) = item
            && digest_name(&type_name(&implementation.self_ty))
                && implementation.trait_.as_ref().is_some_and(|(_, path, _)| {
                    matches!(name(path).rsplit("::").next(), Some("From" | "TryFrom"))
                })
            {
                self.add("digest-conversion-review");
            }
        if let Item::Fn(f) = item {
            let lower = f.sig.ident.to_string().to_ascii_lowercase();
            if digest_name(&lower) && !lower.starts_with("from_") && !lower.starts_with("as_") {
                self.add("digest-function-review");
            }
            if lower.contains("canonical")
                && (lower.contains("encode")
                    || lower.contains("write")
                    || lower.contains("bytes")
                    || lower.contains("fingerprint"))
            {
                self.add("canonical-writer-review");
            }
        }
        visit::visit_item(self, item);
        self.scope.pop();
        self.test = was_test;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let was_test = self.test;
        self.test |= test_attrs(&item.attrs);
        self.scope.push(item.sig.ident.to_string());
        self.attrs(&item.attrs);
        if digest_name(&item.sig.ident.to_string()) {
            self.add("digest-function-review");
        }
        if canonical_writer(&item.sig.ident.to_string()) {
            self.add("canonical-writer-review");
        }
        visit::visit_impl_item_fn(self, item);
        self.scope.pop();
        self.test = was_test;
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if crypto_import(&item.tree, "") {
            self.add("crypto-import-review");
        }
        visit::visit_item_use(self, item);
    }

    fn visit_use_tree(&mut self, tree: &'ast syn::UseTree) {
        let imported = match tree {
            syn::UseTree::Name(import) => Some(import.ident.to_string()),
            syn::UseTree::Rename(import) => Some(import.ident.to_string()),
            _ => None,
        };
        if imported.as_deref().is_some_and(digest_name) {
            self.add("digest-import-review");
        }
        visit::visit_use_tree(self, tree);
    }

    fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
        if digest_name(&name(&ty.path)) {
            self.add("digest-type-review");
        }
        visit::visit_type_path(self, ty);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        let was_test = self.test;
        self.test |= test_attrs(&item.attrs);
        self.scope.push(item.sig.ident.to_string());
        self.attrs(&item.attrs);
        if digest_name(&item.sig.ident.to_string()) {
            self.add("digest-interface-review");
        }
        if canonical_writer(&item.sig.ident.to_string()) {
            self.add("canonical-writer-review");
        }
        visit::visit_trait_item_fn(self, item);
        self.scope.pop();
        self.test = was_test;
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if self.error.is_some() {
            return;
        }
        if self.depth >= 128 {
            self.error = Some("AST expression depth exceeded".into());
            return;
        }
        self.depth += 1;
        match expr {
            Expr::Call(call) => {
                let callee = expr_name(&call.func);
                let segments: Vec<_> = callee.split("::").collect();
                let last = segments.last().copied().unwrap_or_default();
                if matches!(
                    last,
                    "from_bytes"
                        | "from_stored_bytes"
                        | "from_computed_bytes"
                        | "from_hex"
                        | "from_validated"
                ) {
                    if segments
                        .iter()
                        .any(|part| digest_name(part) || part.ends_with("Generation"))
                    {
                        self.add("raw-digest-construction");
                    } else {
                        self.add("raw-construction-review");
                    }
                } else if matches!(
                    last,
                    "Blake3Digest32" | "Sha256Digest32" | "VersionedContentDigest"
                ) {
                    self.add("raw-digest-construction");
                }
                if crypto_name(&callee) || callee.starts_with("Sha256::") {
                    self.add("crypto-call-review");
                }
                // Names are candidates, never dataflow or algorithm proof.
                if digest_name(last)
                    && !matches!(
                        last,
                        "from_bytes" | "from_stored_bytes" | "from_computed_bytes" | "from_hex"
                    )
                {
                    self.add("named-compute-review");
                }
            }
            Expr::MethodCall(call) => {
                let method = call.method.to_string();
                if matches!(
                    method.as_str(),
                    "wrapping_mul" | "rotate_left" | "rotate_right"
                ) {
                    self.add("mixer-review");
                }
                if matches!(method.as_str(), "finalize" | "finalize_reset") {
                    self.add("finalize-review");
                }
                if matches!(method.as_str(), "from_bytes" | "from_stored_bytes") {
                    self.add("unresolved-method-construction");
                }
                if matches!(method.as_str(), "take" | "truncate")
                    && call.args.first().is_some_and(short_bound)
                {
                    self.add("short-prefix-review");
                }
                if method == "split_first"
                    || (matches!(
                        method.as_str(),
                        "split_at" | "split_at_checked" | "split_at_mut"
                    ) && call.args.first().is_some_and(short_bound))
                {
                    self.add("short-prefix-review");
                }
            }
            Expr::Index(index) => {
                if let Expr::Range(range) = index.index.as_ref() {
                    if range.end.as_deref().is_some_and(short_bound) {
                        self.add("digest-prefix-review");
                    } else if range.start.is_none() && range.end.is_some() {
                        self.add("unresolved-prefix-review");
                    }
                }
            }
            Expr::Struct(value) if name(&value.path).ends_with("VersionedContentDigest") => {
                self.add("tagged-digest-literal");
            }
            _ => {}
        }
        visit::visit_expr(self, expr);
        self.depth -= 1;
    }

    fn visit_lit_int(&mut self, literal: &'ast syn::LitInt) {
        if let Ok(value) = literal.base10_parse::<u64>()
            && matches!(
                value,
                0x6a09_e667
                    | 0x428a_2f98
                    | 0xbb67_ae85
                    | 0xcbf2_9ce4_8422_2325
                    | 0x0100_0000_01b3
                    | 2_166_136_261
                    | 16_777_619
            )
        {
            self.add("algorithm-constant-review");
        }
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if macro_relevant(mac.tokens.clone()) || digest_name(&name(&mac.path)) {
            self.add("digest-macro-review");
        } else if !matches!(
            name(&mac.path).as_str(),
            "vec"
                | "format"
                | "format_args"
                | "println"
                | "eprintln"
                | "print"
                | "eprint"
                | "write"
                | "writeln"
                | "assert"
                | "assert_eq"
                | "assert_ne"
                | "debug_assert"
                | "debug_assert_eq"
                | "debug_assert_ne"
                | "matches"
                | "panic"
                | "unreachable"
                | "todo"
                | "include_str"
                | "include_bytes"
                | "env"
                | "option_env"
                | "cfg"
                | "concat"
                | "stringify"
                | "file"
                | "line"
                | "column"
                | "json"
                | "serde_json::json"
        ) {
            self.add("unexpanded-macro-review");
        }
        // Arbitrary macro expansion cannot be inferred from tokens or callee.
    }
}

fn short_bound(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Int(n) if n.base10_parse::<usize>().is_ok_and(|n| n > 0 && n < 32)))
}

fn canonical_writer(name: &str) -> bool {
    name.contains("canonical")
        && (name.contains("encode")
            || name.contains("write")
            || name.contains("bytes")
            || name.contains("fingerprint"))
}

fn macro_relevant(tokens: proc_macro2::TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        proc_macro2::TokenTree::Ident(ident) => digest_name(&ident.to_string()),
        proc_macro2::TokenTree::Group(group) => macro_relevant(group.stream()),
        proc_macro2::TokenTree::Literal(lit) => syn::parse_str::<syn::LitInt>(&lit.to_string())
            .is_ok_and(|n| {
                n.base10_parse::<u64>().is_ok_and(|n| {
                    matches!(
                        n,
                        0x6a09_e667 | 0x428a_2f98 | 0xcbf2_9ce4_8422_2325 | 0x0100_0000_01b3
                    )
                })
            }),
        proc_macro2::TokenTree::Punct(_) => false,
    })
}

fn type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => name(&path.path),
        _ => "<unresolved-type>".into(),
    }
}

fn use_name(tree: &syn::UseTree) -> String {
    match tree {
        syn::UseTree::Path(path) => format!("{}::{}", path.ident, use_name(&path.tree)),
        syn::UseTree::Name(name) => name.ident.to_string(),
        syn::UseTree::Rename(rename) => format!("{} as {}", rename.ident, rename.rename),
        syn::UseTree::Glob(_) => "*".into(),
        syn::UseTree::Group(_) => "{group}".into(),
    }
}
