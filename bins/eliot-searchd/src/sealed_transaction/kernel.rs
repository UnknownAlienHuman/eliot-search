//! Sealed transaction composition behind the stable module facade.

#[path = "kernel/api.rs"]
mod api;
#[path = "kernel/model.rs"]
mod model;
#[path = "kernel/platform.rs"]
mod platform;
#[path = "kernel/spec.rs"]
mod spec;

pub use api::{inspect_transaction, put_idempotent, transaction_status};
pub use model::{
    PutDisposition, SealedTransactionReceipt, TransactionBinding,
    TransactionObservation, TransactionStatus,
};
pub use spec::{MAX_OPERATION_ID_BYTES, SealedTransactionError};

#[cfg(test)]
#[path = "kernel/tests.rs"]
mod tests;
