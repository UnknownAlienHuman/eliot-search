//! Fresh registration's explicit compiled-default policy selection.
use crate::{
    AdmissionError, BASELINE_PROFILE, CanonicalAdmissionPolicy, DEFAULT_MAX_FILE_BYTES,
    ValidatedAdmissionConfig, baseline_policy,
};
use search_contracts::{self as c, CanonicalValue as V};

/// Compile only the complete accepted defaults at genesis revision one.
/// Schema validation alone does not select or activate this runtime policy.
pub fn compile_bootstrap_policy_v1(
    config: &ValidatedAdmissionConfig,
) -> Result<CanonicalAdmissionPolicy, AdmissionError> {
    if config.bootstrap_policy_profile() != BASELINE_PROFILE
        || config.max_file_bytes() != DEFAULT_MAX_FILE_BYTES
        || config.allow_generated()
        || config.allow_vendor()
        || config.allow_binary()
    {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    Ok(baseline_policy(1, DEFAULT_MAX_FILE_BYTES))
}

/// Exact C/P values for fresh-root evidence, using the retained owner policy codec.
/// This pure value does not issue native, currentness or root authority.
pub fn bootstrap_policy_evidence_v1(
    config: &ValidatedAdmissionConfig,
) -> Result<(V, V), AdmissionError> {
    let policy = compile_bootstrap_policy_v1(config)?;
    let text = |s: &str| {
        c::CanonicalText::new(s)
            .map(V::Text)
            .map_err(|_| AdmissionError::PolicyConflict)
    };
    let fields = vec![
        text("compiled-defaults")?,
        text(config.bootstrap_policy_profile())?,
        V::U64(config.max_file_bytes()),
        V::Bool(config.allow_generated()),
        V::Bool(config.allow_vendor()),
        V::Bool(config.allow_binary()),
    ];
    let section = c::BoundedList::new(fields)
        .map(V::Array)
        .map_err(|_| AdmissionError::PolicyConflict)?;
    Ok((section, crate::original_envelope::policy_value(&policy)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AcceptedCapabilities, SectionPlatform, compiled_defaults, validate_section};

    #[test]
    fn bootstrap_v1_all_defaults_and_each_nondefault_are_explicit() {
        let platform = SectionPlatform {
            os: "windows".into(),
            arch: "x64".into(),
        };
        let caps = AcceptedCapabilities {
            profiles: vec![BASELINE_PROFILE.into()],
        };
        let input = compiled_defaults();
        let validated = validate_section(&input, &platform, &caps).unwrap();
        let policy = compile_bootstrap_policy_v1(&validated).unwrap();
        assert_eq!(policy, baseline_policy(1, DEFAULT_MAX_FILE_BYTES));
        assert_eq!(policy.revision().get(), 1);
        let (_, value) = bootstrap_policy_evidence_v1(&validated).unwrap();
        assert_eq!(
            value,
            crate::original_envelope::policy_value(&policy).unwrap()
        );
        for field in 0..5 {
            let mut other = input.clone();
            match field {
                0 => other.bootstrap_policy_profile = "unsupported".into(),
                1 => other.max_file_bytes -= 1,
                2 => other.allow_generated = true,
                3 => other.allow_vendor = true,
                _ => other.allow_binary = true,
            }
            match validate_section(&other, &platform, &caps) {
                Ok(section) => assert_eq!(
                    compile_bootstrap_policy_v1(&section),
                    Err(AdmissionError::PolicySchemaUnsupported)
                ),
                Err(error) => assert_eq!(error, AdmissionError::PolicySchemaUnsupported),
            }
        }
    }
}
