//! Windows sealed-transaction composition.

mod codec;
mod io;
mod model;
mod put;
mod status;

pub(crate) use put::put_idempotent;
pub(crate) use status::{inspect_transaction, transaction_status};

#[cfg(test)]
mod tests;
