//! Catalog-only command under the invocation's existing native inspection custody.

use std::ffi::OsString;
use std::io::{self, Write};

use crate::owner_composition::NativeReadOperation;
use crate::service_output::json_string;

/// Arity refusal precedes root admission and preserves the legacy error frame.
pub(super) fn emit_usage_refusal(
    arguments: &[OsString],
    writer: &mut impl Write,
) -> io::Result<bool> {
    let Ok((selected, _)) = crate::config_composition::strip_config_args_os(arguments) else {
        return Ok(false);
    };
    if selected
        .first()
        .is_none_or(|command| command != "--source-roots")
        || selected.len() == 2
    {
        return Ok(false);
    }
    writeln!(writer, "{{\"error\":\"SOURCE_ROOT_USAGE_ERROR\"}}")?;
    writer.flush()?;
    Ok(true)
}

pub(super) fn execute(operation: &mut NativeReadOperation<'_>) -> Result<(), String> {
    let result = crate::source_roots::SourceRootInspection::read(operation)
        .and_then(|snapshot| snapshot.output());
    match result {
        Ok(bytes) => operation.emit_registration(&mut io::stdout().lock(), &bytes),
        Err(error) => {
            if !operation.output_attempted() && operation.checkpoint().is_ok() {
                let line = format!(
                    "{{\"error\":{}}}\n",
                    json_string(&eliot_searchd::diagnostics::sanitize_code(&error))
                );
                operation.emit_registration(&mut io::stderr().lock(), line.as_bytes())?;
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_arity_preserves_error_only_stderr_before_admission() {
        for values in [
            vec!["--source-roots"],
            vec!["--source-roots", "private-root", "extra"],
            vec!["--config-file", "private-config", "--source-roots"],
        ] {
            let arguments: Vec<_> = values.into_iter().map(OsString::from).collect();
            let mut output = Vec::new();
            assert_eq!(emit_usage_refusal(&arguments, &mut output).unwrap(), true);
            assert_eq!(output, b"{\"error\":\"SOURCE_ROOT_USAGE_ERROR\"}\n");
        }
        for values in [
            vec!["--source-roots", "private-root"],
            vec![
                "--config-file",
                "private-config",
                "--source-roots",
                "private-root",
            ],
            vec!["--health-data-root"],
        ] {
            let arguments: Vec<_> = values.into_iter().map(OsString::from).collect();
            let mut output = Vec::new();
            assert!(!emit_usage_refusal(&arguments, &mut output).unwrap());
            assert!(output.is_empty());
        }
    }
}
