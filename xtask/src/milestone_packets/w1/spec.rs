pub(super) struct PackageSpec {
    pub(super) name: &'static str,
    pub(super) path: &'static str,
    pub(super) deps: &'static [&'static str],
    pub(super) milestones: &'static [&'static str],
}

pub(super) const PACKAGES: [PackageSpec; 7] = [
    PackageSpec {
        name: "search-config",
        path: "crates/search-config",
        deps: &["search-contracts"],
        milestones: &["C0", "C1", "C2", "C3"],
    },
    PackageSpec {
        name: "search-runtime-owner",
        path: "crates/search-runtime/search-runtime-owner",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["R0", "R1", "R2", "R3"],
    },
    PackageSpec {
        name: "search-os-secrets",
        path: "crates/search-runtime/search-os-secrets",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["S0", "S1", "S2", "S3"],
    },
    PackageSpec {
        name: "search-control-redb",
        path: "crates/search-control-redb",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["J0", "J1", "J2", "J3"],
    },
    PackageSpec {
        name: "search-provider-protocol",
        path: "crates/search-provider-protocol",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["P0", "P1", "P2", "P3"],
    },
    PackageSpec {
        name: "eliot-searchd",
        path: "bins/eliot-searchd",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
            "search-runtime-owner",
            "search-os-secrets",
            "search-control-redb",
            "search-provider-protocol",
        ],
        milestones: &["D0", "D1", "D2", "D3"],
    },
    PackageSpec {
        name: "eliot-search",
        path: "bins/eliot-search",
        deps: &[
            "search-contracts",
            "search-ports",
            "search-config",
            "search-provider-protocol",
        ],
        milestones: &["L0", "L1", "L2", "L3"],
    },
];
