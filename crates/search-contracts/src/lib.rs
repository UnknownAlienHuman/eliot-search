#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]
// The names and several intentionally wide tagged records mirror the accepted
// P00 wire vocabulary. Boxing or renaming them solely for style would change
// the public contract surface.
#![allow(
    clippy::large_enum_variant,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::struct_excessive_bools,
    clippy::too_many_lines
)]

macro_rules! impl_wire_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            pub fn parse(value: &str) -> Result<Self, $crate::ContractError> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err($crate::ContractError::new(
                        $crate::ContractErrorKind::InvalidCharacter,
                        stringify!($name),
                    )),
                }
            }
        }
    };
}

pub(crate) use impl_wire_enum;

pub mod bounds;
pub mod canonical;
pub mod error;
pub mod ids;
pub mod lifecycle;
pub mod protocol;
pub mod query;
pub mod reasons;
pub mod recipes;
pub mod results;
pub mod schema;
pub mod source;

pub use bounds::*;
pub use canonical::*;
pub use error::*;
pub use ids::*;
pub use lifecycle::*;
pub use protocol::*;
pub use query::*;
pub use reasons::*;
pub use recipes::*;
pub use results::*;
pub use schema::*;
pub use source::*;
