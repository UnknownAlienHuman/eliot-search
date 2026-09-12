//! Bounded daemon-owned path classification.
//!
//! Paths remain locators, never durable identity. This classifier emits only
//! the closed, content-free signals consumed by `search-source-admission`.

use std::path::{Component, Path};

use search_source_admission::{SensitivityLevel, SourceClass};

const MAX_CLASSIFIER_PATH_BYTES: usize = 4_096;
const MAX_CLASSIFIER_COMPONENTS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ClassifiedSource {
    pub(super) source_class: SourceClass,
    pub(super) sensitivity: SensitivityLevel,
    pub(super) is_generated: bool,
    pub(super) is_vendor: bool,
    pub(super) is_binary: bool,
    pub(super) is_system: bool,
}

pub(super) fn classify(path: &Path) -> Result<ClassifiedSource, String> {
    let raw_name = file_name_text(path)?;
    let name = raw_name.to_ascii_lowercase();
    let components = path_components_lower(path)?;
    let extension = raw_name
        .rfind('.')
        .map_or("", |index| {
            if index + 1 < raw_name.len() {
                &raw_name[index + 1..]
            } else {
                ""
            }
        })
        .to_ascii_lowercase();

    let (source_class, sensitivity, binary_hint) = credential_class(&name, &extension)
        .or_else(|| system_cache_build_class(&name, &components, &extension))
        .or_else(|| generated_vendor_class(&name, &components, &extension))
        .or_else(|| binary_class(&name, &extension))
        .or_else(|| secret_candidate_class(&name))
        .unwrap_or_else(|| baseline_class(&name, &components, &extension));

    Ok(ClassifiedSource {
        source_class,
        sensitivity,
        is_generated: source_class == SourceClass::Generated,
        is_vendor: source_class == SourceClass::Vendor,
        is_binary: binary_hint || source_class == SourceClass::Binary,
        is_system: matches!(
            source_class,
            SourceClass::System | SourceClass::Cache | SourceClass::BuildOutput
        ),
    })
}

fn file_name_text(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_SOURCE_PATH_DENIED".to_owned())?;
    if name.is_empty() || name.len() > MAX_CLASSIFIER_PATH_BYTES || name.contains('\0') {
        return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
    }
    Ok(name.to_owned())
}

fn path_components_lower(path: &Path) -> Result<Vec<String>, String> {
    let mut output = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let text = part
                    .to_str()
                    .ok_or_else(|| "DIRECT_SOURCE_PATH_DENIED".to_owned())?;
                if text.is_empty() || text.len() > MAX_CLASSIFIER_PATH_BYTES {
                    return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
                }
                output.push(text.to_ascii_lowercase());
                if output.len() > MAX_CLASSIFIER_COMPONENTS {
                    return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
                }
            }
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => {}
        }
    }
    if output.is_empty() {
        return Err("DIRECT_SOURCE_PATH_DENIED".to_owned());
    }
    Ok(output)
}

fn credential_class(
    name: &str,
    extension: &str,
) -> Option<(SourceClass, SensitivityLevel, bool)> {
    let credential_name = name == "id_rsa"
        || name == "id_ed25519"
        || name == "id_ecdsa"
        || name == "id_dsa"
        || name.starts_with("id_rsa.")
        || name.starts_with("id_ed25519.")
        || name == "private_key"
        || name == "private-key"
        || name == "secret_key"
        || name == "secret-key"
        || matches!(
            extension,
            "pem" | "key" | "pfx" | "p12" | "asc" | "gpg" | "pgp" | "kdbx"
        )
        || matches!(
            name,
            "credentials.json"
                | "secrets.yaml"
                | "secrets.yml"
                | "secrets.json"
                | "secrets.toml"
        );
    if !credential_name {
        return None;
    }
    if matches!(extension, "pem" | "key" | "pfx" | "p12")
        || name.starts_with("id_")
        || name.contains("private")
    {
        Some((
            SourceClass::PrivateKey,
            SensitivityLevel::PrivateKey,
            false,
        ))
    } else {
        Some((
            SourceClass::Credential,
            SensitivityLevel::Credential,
            false,
        ))
    }
}

fn system_cache_build_class(
    name: &str,
    components: &[String],
    extension: &str,
) -> Option<(SourceClass, SensitivityLevel, bool)> {
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            ".git"
                | ".hg"
                | ".svn"
                | "system volume information"
                | "$recycle.bin"
                | ".ds_store"
        )
    }) || matches!(name, ".ds_store" | "thumbs.db")
    {
        return Some((SourceClass::System, SensitivityLevel::Internal, false));
    }
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "__pycache__" | ".cache" | ".mypy_cache" | ".pytest_cache" | ".venv" | "venv"
        )
    }) {
        return Some((SourceClass::Cache, SensitivityLevel::Internal, false));
    }
    if components
        .iter()
        .any(|component| matches!(component.as_str(), "target" | "dist" | "build" | "out"))
        || matches!(extension, "o" | "obj" | "class" | "pyc" | "pyo")
    {
        return Some((
            SourceClass::BuildOutput,
            SensitivityLevel::Internal,
            false,
        ));
    }
    None
}

fn generated_vendor_class(
    name: &str,
    components: &[String],
    extension: &str,
) -> Option<(SourceClass, SensitivityLevel, bool)> {
    if name.contains(".generated.")
        || name.contains("_generated")
        || name.contains("generated_")
        || components.iter().any(|component| component == "generated")
        || matches!(extension, "g.cs" | "pb.go")
        || name.ends_with(".designer.cs")
        || name.ends_with(".min.js")
    {
        return Some((
            SourceClass::Generated,
            SensitivityLevel::Internal,
            false,
        ));
    }
    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            "vendor" | "third_party" | "thirdparty" | "node_modules"
        )
    }) {
        return Some((SourceClass::Vendor, SensitivityLevel::Internal, false));
    }
    None
}

fn binary_class(
    name: &str,
    extension: &str,
) -> Option<(SourceClass, SensitivityLevel, bool)> {
    let binary_hint = matches!(
        extension,
        "exe"
            | "dll"
            | "so"
            | "dylib"
            | "bin"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "ico"
            | "zip"
            | "gz"
            | "7z"
            | "rar"
            | "pdf"
    );
    if !binary_hint {
        return None;
    }
    let sensitivity = if name.contains("secret")
        || name.contains("credential")
        || name.contains("token")
        || name.contains("password")
        || name.contains("private")
    {
        SensitivityLevel::SecretCandidate
    } else {
        SensitivityLevel::Internal
    };
    Some((SourceClass::Binary, sensitivity, true))
}

fn secret_candidate_class(
    name: &str,
) -> Option<(SourceClass, SensitivityLevel, bool)> {
    if name.contains("secret")
        || name.contains("credential")
        || name.contains("private")
        || name.contains("password")
        || name == ".env"
        || name.starts_with(".env.")
        || (name.contains("token") && (name.contains("api") || name.contains("auth")))
    {
        return Some((
            SourceClass::Regular,
            SensitivityLevel::SecretCandidate,
            false,
        ));
    }
    None
}

fn baseline_class(
    name: &str,
    components: &[String],
    extension: &str,
) -> (SourceClass, SensitivityLevel, bool) {
    let documentation = extension == "md"
        || extension == "rst"
        || name.starts_with("readme")
        || name.starts_with("changelog")
        || name.starts_with("license")
        || components
            .iter()
            .any(|component| matches!(component.as_str(), "docs" | "documentation"));
    if documentation {
        return (SourceClass::Documentation, SensitivityLevel::Public, false);
    }
    let test = components.iter().any(|component| {
        matches!(component.as_str(), "test" | "tests" | "testing")
    }) || name.starts_with("test_")
        || name.starts_with("test-")
        || name.ends_with("_test.rs")
        || name.ends_with("_test.go")
        || name.ends_with(".test.js")
        || name.ends_with(".test.ts");
    if test {
        return (SourceClass::Test, SensitivityLevel::Internal, false);
    }
    (SourceClass::Regular, SensitivityLevel::Internal, false)
}
