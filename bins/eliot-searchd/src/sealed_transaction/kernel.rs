//! Sealed transaction composition behind the stable module facade.

mod api;
mod model;
mod platform;
mod spec;

pub use api::{inspect_transaction, put_idempotent, transaction_status};
pub use model::{
    PutDisposition, SealedTransactionReceipt, TransactionBinding,
    TransactionObservation, TransactionStatus,
};
pub use spec::{MAX_OPERATION_ID_BYTES, SealedTransactionError};

#[cfg(test)]
mod tests;
