//! Effective-configuration health and status projections for commands.

use crate::development::Health;

pub(super) fn shell_health_effective() -> Result<Health, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (_, cli) = crate::config_composition::parse_cli_config_args(&args)?;
    let effective = crate::config_composition::effective_from_process(&cli)?;
    let report = crate::config_composition::derive_readiness(
        &effective,
        crate::config_composition::shell_dependencies(),
        crate::config_composition::AcceptedReceipts::default(),
    );
    Ok(Health::from_readiness(&report))
}

pub(super) fn direct_health_effective() -> Result<Health, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (_, cli) = crate::config_composition::parse_cli_config_args(&args)?;
    let effective = crate::config_composition::effective_from_process(&cli)?;
    let report = crate::config_composition::derive_readiness(
        &effective,
        crate::config_composition::direct_dependencies(),
        crate::config_composition::AcceptedReceipts::default(),
    );
    Ok(Health::from_readiness(&report))
}

pub(super) fn config_status_line() -> Result<String, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (_, cli) = crate::config_composition::parse_cli_config_args(&args)?;
    let effective = crate::config_composition::effective_from_process(&cli)?;
    let report = crate::config_composition::derive_readiness(
        &effective,
        crate::config_composition::shell_dependencies(),
        crate::config_composition::AcceptedReceipts::default(),
    );
    Ok(crate::config_composition::config_status_json(
        &effective,
        &report,
    ))
}
