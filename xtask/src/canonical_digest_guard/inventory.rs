use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};
use syn::visit::Visit;

use super::{Key, Site, Sites, detector::Detector, scan};

pub(super) fn collect(root: &Path) -> Result<Sites, String> {
    let mut budget = scan::Budget::default();
    let files = scan::files(root, &mut budget)?;
    let root_text = scan::read(&root.join("Cargo.toml"), &mut budget)?;
    let root_manifest =
        manifest_toml::from_str::<manifest_toml::Value>(&root_text).map_err(|e| e.to_string())?;
    let aliases: BTreeMap<String, String> = root_manifest
        .get("workspace")
        .and_then(|v| v.get("dependencies"))
        .and_then(manifest_toml::Value::as_table)
        .into_iter()
        .flatten()
        .map(|(alias, spec)| {
            (
                alias.clone(),
                spec.get("package")
                    .and_then(manifest_toml::Value::as_str)
                    .unwrap_or(alias)
                    .to_owned(),
            )
        })
        .collect();
    let mut sites = Sites::new();
    for path in files {
        let label = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("non-UTF8 source path")?
            .replace('\\', "/");
        let source = scan::read(&path, &mut budget).map_err(|e| format!("{label}: {e}"))?;
        // Git text checkouts may use CRLF. Bind every full source byte after
        // the same LF normalization, so checkout policy cannot stale the ledger.
        let normalized = source.replace("\r\n", "\n");
        let sha256 = hex(&Sha256::digest(normalized.as_bytes()));
        if path.extension().is_some_and(|ext| ext == "rs") {
            let tokens = super::syntax::tokens(&source).map_err(|e| format!("{label}: {e}"))?;
            let file =
                syn::parse2::<syn::File>(tokens).map_err(|e| format!("{label}: Rust AST: {e}"))?;
            let mut detector = Detector::new(label.split('/').any(|part| part == "tests"));
            detector.visit_file(&file);
            if let Some(error) = detector.error {
                return Err(format!("{label}: {error}"));
            }
            for ((symbol, signal, test), count) in detector.findings {
                let key = Key {
                    path: label.clone(),
                    symbol,
                    signal,
                };
                if let Some(site) = sites.get_mut(&key) {
                    site.count += count;
                    site.test &= test;
                } else {
                    sites.insert(
                        key.clone(),
                        Site {
                            key,
                            count,
                            test,
                            source_sha256: sha256.clone(),
                        },
                    );
                }
            }
        } else {
            let manifest = manifest_toml::from_str::<manifest_toml::Value>(&source)
                .map_err(|e| format!("{label}: TOML: {e}"))?;
            dependencies(&manifest, "", &label, &sha256, &aliases, &mut sites);
        }
        if sites.len() > 32_768 {
            return Err("repository finding limit exceeded".into());
        }
    }
    Ok(sites)
}

fn dependencies(
    value: &manifest_toml::Value,
    table: &str,
    path: &str,
    sha256: &str,
    aliases: &BTreeMap<String, String>,
    sites: &mut Sites,
) {
    let Some(entries) = value.as_table() else {
        return;
    };
    for (name, value) in entries {
        let nested = if table.is_empty() {
            name.clone()
        } else {
            format!("{table}.{name}")
        };
        if matches!(
            name.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            if let Some(deps) = value.as_table() {
                for (alias, spec) in deps {
                    let inherited = spec
                        .get("workspace")
                        .and_then(manifest_toml::Value::as_bool)
                        == Some(true);
                    let actual = spec
                        .get("package")
                        .and_then(manifest_toml::Value::as_str)
                        .unwrap_or_else(|| {
                            if inherited {
                                aliases.get(alias).map_or(alias.as_str(), String::as_str)
                            } else {
                                alias
                            }
                        });
                    if crypto_codec(actual) {
                        let key = Key {
                            path: path.into(),
                            symbol: format!("{nested}.{alias}[{actual}]"),
                            signal: "dependency-review".into(),
                        };
                        sites.insert(
                            key.clone(),
                            Site {
                                key,
                                count: 1,
                                test: name == "dev-dependencies",
                                source_sha256: sha256.into(),
                            },
                        );
                    }
                }
            }
        } else {
            dependencies(value, &nested, path, sha256, aliases, sites);
        }
    }
}

fn crypto_codec(name: &str) -> bool {
    matches!(
        name,
        "blake3"
            | "sha2"
            | "sha3"
            | "digest"
            | "ring"
            | "openssl"
            | "hmac"
            | "md-5"
            | "md5"
            | "sha1"
            | "xxhash-rust"
            | "twox-hash"
            | "fnv"
            | "ciborium"
            | "serde_cbor"
            | "cbor4ii"
            | "minicbor"
            | "bincode"
            | "postcard"
            | "serde_json"
            | "toml"
            | "toml_edit"
            | "jcs"
            | "serde_jcs"
            | "canonical_json"
            | "chacha20poly1305"
            | "aes-gcm"
    ) || name.contains("canonical")
}

pub(super) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("string write");
    }
    output
}
