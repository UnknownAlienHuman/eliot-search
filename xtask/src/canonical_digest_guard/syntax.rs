use proc_macro2::{TokenStream, TokenTree};
use syn::{Attribute, Expr, UseTree};

pub(super) fn tokens(source: &str) -> Result<TokenStream, String> {
    // The exact proc-macro2 fallback lexer uses an iterative group stack.
    // The physical read is capped first; token/group/operator budgets below
    // are checked before syn constructs or recursively visits an AST.
    let tokens: TokenStream = source.parse().map_err(|e| format!("Rust tokens: {e}"))?;
    let mut remaining = 250_000_usize;
    check_tokens(tokens.clone(), 0, &mut remaining)?;
    Ok(tokens)
}

fn check_tokens(tokens: TokenStream, depth: usize, remaining: &mut usize) -> Result<(), String> {
    if depth > 64 {
        return Err("token depth exceeded".into());
    }
    let mut operator_run = 0_usize;
    for token in tokens {
        if *remaining == 0 {
            return Err("token budget exceeded".into());
        }
        *remaining -= 1;
        match token {
            TokenTree::Group(group) => {
                check_tokens(group.stream(), depth + 1, remaining)?;
                operator_run = 0;
            }
            TokenTree::Punct(p) if matches!(p.as_char(), ';' | ',') => operator_run = 0,
            TokenTree::Punct(_) => {
                operator_run += 1;
                if operator_run > 128 {
                    return Err("operator-chain budget exceeded".into());
                }
            }
            TokenTree::Ident(ident) if ident == "as" => {
                operator_run += 1;
                if operator_run > 128 {
                    return Err("cast-chain budget exceeded".into());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn test_attrs(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || (attr.path().is_ident("cfg")
                && attr
                    .meta
                    .require_list()
                    .is_ok_and(|list| list.tokens.to_string() == "test"))
    })
}

pub(super) fn name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn digest_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("digest")
        || lower.contains("fingerprint")
        || lower.contains("sha256")
        || lower.contains("blake3")
        || lower.contains("hash")
}

pub(super) fn expr_name(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => name(&path.path),
        Expr::Field(field) => expr_name(&field.base),
        Expr::MethodCall(call) => format!("{}::{}", expr_name(&call.receiver), call.method),
        Expr::Call(call) => expr_name(&call.func),
        Expr::Reference(reference) => expr_name(&reference.expr),
        Expr::Paren(paren) => expr_name(&paren.expr),
        _ => String::new(),
    }
}

pub(super) fn crypto_import(tree: &UseTree, prefix: &str) -> bool {
    match tree {
        UseTree::Path(path) => crypto_import(&path.tree, &format!("{prefix}{}::", path.ident)),
        UseTree::Group(group) => group.items.iter().any(|tree| crypto_import(tree, prefix)),
        UseTree::Name(name) => crypto_name(&format!("{prefix}{}", name.ident)),
        UseTree::Rename(rename) => crypto_name(&format!("{prefix}{}", rename.ident)),
        UseTree::Glob(_) => crypto_name(prefix),
    }
}

pub(super) fn crypto_name(name: &str) -> bool {
    name.starts_with("blake3::")
        || name.starts_with("sha2::")
        || name.starts_with("sha3::")
        || name.starts_with("ring::digest")
        || name.starts_with("openssl::")
}
