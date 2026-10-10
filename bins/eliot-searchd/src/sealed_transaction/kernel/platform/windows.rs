//! Windows sealed-transaction composition.

#[path = "windows/codec.rs"]
mod codec;
#[path = "windows/io.rs"]
mod io;
#[path = "windows/model.rs"]
mod model;
#[path = "windows/put.rs"]
mod put;
#[path = "windows/status.rs"]
mod status;

pub(crate) use put::put_idempotent;
pub(crate) use status::{inspect_transaction, transaction_status};

#[cfg(test)]
#[path = "windows/tests.rs"]
mod tests;
