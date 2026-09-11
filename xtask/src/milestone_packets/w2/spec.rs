pub(super) struct PackageSpec {
    pub(super) name: &'static str,
    pub(super) path: &'static str,
    pub(super) deps: &'static [&'static str],
    pub(super) milestones: &'static [&'static str],
}

pub(super) const PACKAGES: [PackageSpec; 8] = [
    PackageSpec {
        name: "search-source-admission",
        path: "crates/search-source/search-source-admission",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["A0", "A1", "A2", "A3"],
    },
    PackageSpec {
        name: "search-source-identity",
        path: "crates/search-source/search-source-identity",
        deps: &["search-contracts", "search-domain"],
        milestones: &["I0", "I1", "I2", "I3"],
    },
    PackageSpec {
        name: "search-safe-reader",
        path: "crates/search-source/search-safe-reader",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["SR0", "SR1", "SR2", "SR3"],
    },
    PackageSpec {
        name: "search-revision-store",
        path: "crates/search-source/search-revision-store",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["V0", "V1", "V2", "V3"],
    },
    PackageSpec {
        name: "search-materializer",
        path: "crates/search-prep/search-materializer",
        deps: &["search-contracts", "search-domain", "search-ports"],
        milestones: &["M0", "M1", "M2", "M3"],
    },
    PackageSpec {
        name: "search-unitizer",
        path: "crates/search-prep/search-unitizer",
        deps: &["search-contracts", "search-domain", "search-ports"],
        milestones: &["U0", "U1", "U2", "U3"],
    },
    PackageSpec {
        name: "search-source-registry",
        path: "crates/search-source/search-source-registry",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-source-identity",
            "search-source-admission",
        ],
        milestones: &["RG0", "RG1", "RG2", "RG3"],
    },
    PackageSpec {
        name: "eliot-searchd",
        path: "bins/eliot-searchd",
        deps: &[
            "eliot-searchd",
            "search-source-admission",
            "search-source-registry",
            "search-source-identity",
            "search-safe-reader",
            "search-revision-store",
            "search-materializer",
            "search-unitizer",
        ],
        milestones: &["D20", "D21", "D22", "D23"],
    },
];
