pub(super) struct PackageSpec {
    pub(super) name: &'static str,
    pub(super) path: &'static str,
    pub(super) deps: &'static [&'static str],
    pub(super) milestones: &'static [&'static str],
}

pub(super) const PACKAGES: [PackageSpec; 9] = [
    PackageSpec {
        name: "search-lexical",
        path: "crates/search-lexical",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["LX0", "LX1", "LX2", "LX3"],
    },
    PackageSpec {
        name: "search-point-identity",
        path: "crates/search-index-qdrant/search-point-identity",
        deps: &["search-contracts", "search-domain"],
        milestones: &["PI0", "PI1", "PI2", "PI3"],
    },
    PackageSpec {
        name: "search-qdrant-supervisor",
        path: "crates/search-index-qdrant/search-qdrant-supervisor",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["QS0", "QS1", "QS2", "QS3"],
    },
    PackageSpec {
        name: "search-qdrant-bridge",
        path: "crates/search-index-qdrant/search-qdrant-bridge",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
        ],
        milestones: &["QB0", "QB1", "QB2", "QB3"],
    },
    PackageSpec {
        name: "search-epoch-pins",
        path: "crates/search-index-qdrant/search-epoch-pins",
        deps: &["search-contracts", "search-domain", "search-ports"],
        milestones: &["EP0", "EP1", "EP2", "EP3"],
    },
    PackageSpec {
        name: "search-projection-planner",
        path: "crates/search-index-qdrant/search-projection-planner",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-point-identity",
        ],
        milestones: &["PP0", "PP1", "PP2", "PP3"],
    },
    PackageSpec {
        name: "search-index-reclaimer",
        path: "crates/search-index-qdrant/search-index-reclaimer",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
            "search-epoch-pins",
        ],
        milestones: &["IR0", "IR1", "IR2", "IR3"],
    },
    PackageSpec {
        name: "search-publication",
        path: "crates/search-index-qdrant/search-publication",
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-projection-planner",
            "search-point-identity",
        ],
        milestones: &["PUB0", "PUB1", "PUB2", "PUB3"],
    },
    PackageSpec {
        name: "eliot-searchd",
        path: "bins/eliot-searchd",
        deps: &[
            "eliot-searchd",
            "search-lexical",
            "search-projection-planner",
            "search-point-identity",
            "search-qdrant-supervisor",
            "search-qdrant-bridge",
            "search-publication",
            "search-epoch-pins",
            "search-index-reclaimer",
        ],
        milestones: &["IDX0", "IDX1", "IDX2", "IDX3"],
    },
];
