use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TxError {
    #[error("CSV Error")]
    CsvError(#[from] csv::Error),
    #[error("I/O Error")]
    IoError(#[from] io::Error),
    #[error("Insufficient Funds")]
    InsufficientFunds,
    #[error("Missing amount in transaction data")]
    MissingAmount,
    #[error("Bad dispute")]
    BadDispute,
    #[error("Deposits and withdrawals must be positive amounts")]
    InvalidAmount,
    #[error("The account is locked")]
    LockedAccount,
    #[error("Given transaction could not be found")]
    NotFound,
    #[error("Tried to mutate a transaction not owned by you")]
    InsufficientPermission,
    #[error("Unknown error")]
    Unknown,
}
