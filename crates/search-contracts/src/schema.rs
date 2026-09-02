use crate::{ContractError, ContractErrorKind};

macro_rules! wire_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            pub fn parse(value: &str) -> Result<Self, ContractError> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(ContractError::new(
                        ContractErrorKind::InvalidCharacter,
                        stringify!($name),
                    )),
                }
            }
        }
    };
}

wire_enum!(AssuranceClass {
    ExactBytes => "exact_bytes",
    MappedText => "mapped_text",
    LossyText => "lossy_text",
    DescriptiveOnly => "descriptive_only",
});

wire_enum!(ObservationFreshnessState {
    CurrentConfirmed => "current_confirmed",
    ObservedWithAge => "observed_with_age",
    GapDetected => "gap_detected",
    Unknown => "unknown",
});

wire_enum!(EvidenceRole {
    Definition => "definition",
    Reference => "reference",
    Test => "test",
    Documentation => "documentation",
    Caller => "caller",
    Configuration => "configuration",
});

wire_enum!(Modality {
    Code => "code",
    Text => "text",
    Document => "document",
    Image => "image",
    Archive => "archive",
    Mixed => "mixed",
});

wire_enum!(EntityKind {
    Function => "function",
    Method => "method",
    Type => "type",
    Trait => "trait",
    Impl => "impl",
    Module => "module",
    Field => "field",
    Constant => "constant",
    Static => "static",
    Macro => "macro",
    Variable => "variable",
    Parameter => "parameter",
    File => "file",
    Section => "section",
    Test => "test",
    Document => "document",
    Table => "table",
    ImageRegion => "image_region",
    Unknown => "unknown",
});

wire_enum!(SensitivityClass {
    Public => "public",
    Project => "project",
    Confidential => "confidential",
    SecretCandidate => "secret_candidate",
});

wire_enum!(DisclosureCeiling {
    LocalOnly => "local_only",
    NamedClient => "named_client",
    Exportable => "exportable",
});
