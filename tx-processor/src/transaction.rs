use std::collections::HashMap;
use std::convert::TryFrom;

use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};

use crate::account::Account;
use crate::error::TxError;

pub const NUM_DECIMAL_PLACES: u32 = 4;

#[allow(clippy::module_name_repetitions)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionRecordType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize)]
pub struct TransactionRecord {
    #[serde(rename = "type")]
    pub transaction_type: TransactionRecordType,
    #[serde(rename = "client")]
    pub client_id: u16,
    #[serde(rename = "tx")]
    pub transaction_id: u32,
    pub amount: Option<Decimal>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Transaction {
    pub client_id: u16,
    pub transaction_id: u32,
    pub tx_type: TransactionType,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq, Eq)]
pub enum TransactionType {
    Deposit { amount: PositiveDecimal },
    Withdrawal { amount: PositiveDecimal },
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PositiveDecimal(Decimal);

pub trait Transact {
    /// # Errors
    /// Errors when the given `amount` would cause an overflow
    fn deposit(&mut self, amount: PositiveDecimal) -> Result<(), TxError>;

    /// # Errors
    /// Errors when the given `amount` would cause an underflow/be negative
    fn withdraw(&mut self, amount: PositiveDecimal) -> Result<(), TxError>;

    /// # Errors
    /// Errors when the dispute is not a valid transaction.
    /// 1. The `disputed_tx_id` is not owned by `self`
    /// 2. The `disputed_tx_id` is not in the `transaction_log`
    /// 3. The `disputed_tx_id` is already disputed (ie, in the `disputed_tx_map`)
    fn dispute(
        &mut self,
        disputed_tx_id: u32,
        transaction_log: &[Transaction],
        disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> Result<(), TxError>;

    /// # Errors
    /// Errors when the resolve is not a valid transaction.
    /// 1. The `transaction_id` is not owned by `self`
    /// 2. The `transaction_id` is not in the `disputed_tx_map`
    fn resolve(
        &mut self,
        transaction_id: u32,
        disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> Result<(), TxError>;

    /// # Errors
    /// Errors when the chargeback is not a valid transaction.
    /// 1. The `transaction_id` is not owned by `self`
    /// 2. The `transaction_id` is not in the `disputed_tx_map`
    fn chargeback(
        self,
        transaction_id: u32,
        disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> (Result<Account<true>, TxError>, Option<Account<false>>);
}

impl TryFrom<Decimal> for PositiveDecimal {
    type Error = TxError;
    fn try_from(mut decimal: Decimal) -> Result<Self, Self::Error> {
        if decimal >= Decimal::ZERO {
            decimal.rescale(NUM_DECIMAL_PLACES);
            Ok(PositiveDecimal(decimal))
        } else {
            Err(TxError::InvalidAmount)
        }
    }
}

impl TryFrom<f64> for PositiveDecimal {
    type Error = TxError;
    fn try_from(decimal: f64) -> Result<Self, Self::Error> {
        PositiveDecimal::try_from(Decimal::from_f64(decimal).ok_or(TxError::InvalidAmount)?)
    }
}

impl PositiveDecimal {
    /// # Errors
    /// Errors when `other` + `self` would overflow
    pub fn checked_add(self, other: PositiveDecimal) -> Result<PositiveDecimal, TxError> {
        self.0
            .checked_add(other.0)
            .map(PositiveDecimal)
            .ok_or(TxError::InvalidAmount)
    }

    /// # Errors
    /// Errors when `other` is > `self`, since the resulting number would not be Positive
    pub fn checked_sub(self, other: PositiveDecimal) -> Result<PositiveDecimal, TxError> {
        if self >= other {
            self.0
                .checked_sub(other.0)
                .map(PositiveDecimal)
                .ok_or(TxError::InvalidAmount)
        } else {
            Err(TxError::InsufficientFunds)
        }
    }
}

impl Transaction {
    #[must_use]
    pub fn new(client_id: u16, transaction_id: u32, tx_type: TransactionType) -> Self {
        Transaction {
            client_id,
            transaction_id,
            tx_type,
        }
    }
}

impl TryFrom<TransactionRecord> for Transaction {
    type Error = TxError;
    fn try_from(record: TransactionRecord) -> Result<Self, Self::Error> {
        match record.transaction_type {
            TransactionRecordType::Deposit => {
                let amount = record.amount.map_or(Err(TxError::MissingAmount), |val| {
                    PositiveDecimal::try_from(val)
                })?;
                Ok(Transaction::new(
                    record.client_id,
                    record.transaction_id,
                    TransactionType::Deposit { amount },
                ))
            }
            TransactionRecordType::Withdrawal => {
                let amount = record.amount.map_or(Err(TxError::MissingAmount), |val| {
                    PositiveDecimal::try_from(val)
                })?;
                Ok(Transaction::new(
                    record.client_id,
                    record.transaction_id,
                    TransactionType::Withdrawal { amount },
                ))
            }
            TransactionRecordType::Dispute => Ok(Transaction::new(
                record.client_id,
                record.transaction_id,
                TransactionType::Dispute,
            )),
            TransactionRecordType::Resolve => Ok(Transaction::new(
                record.client_id,
                record.transaction_id,
                TransactionType::Resolve,
            )),
            TransactionRecordType::Chargeback => Ok(Transaction::new(
                record.client_id,
                record.transaction_id,
                TransactionType::Chargeback,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_decimal_try_from() {
        let neg_decimal = Decimal::from_f64(-1.111).unwrap();
        assert!(PositiveDecimal::try_from(neg_decimal).is_err());

        let pos_decimal = Decimal::from_f64(1.111).unwrap();
        assert!(PositiveDecimal::try_from(pos_decimal).is_ok());

        assert!(PositiveDecimal::try_from(Decimal::ZERO).is_ok());

        let long_decimal = PositiveDecimal::try_from(1.123_456).unwrap();
        let short_decimal = PositiveDecimal::try_from(1.123_5).unwrap();
        assert_eq!(long_decimal, short_decimal);

        let long_decimal = PositiveDecimal::try_from(1.654_321).unwrap();
        let short_decimal = PositiveDecimal::try_from(1.654_3).unwrap();
        assert_eq!(long_decimal, short_decimal);
    }

    #[test]
    fn test_positive_decimal_checked_add() {
        let pos_decimal_1 = PositiveDecimal::try_from(1.111).unwrap();
        let pos_decimal_2 = PositiveDecimal::try_from(10.111).unwrap();
        let result = pos_decimal_1.checked_add(pos_decimal_2);
        assert!(result.is_ok());

        let pos_decimal_1 = PositiveDecimal::try_from(Decimal::MAX).unwrap();
        let pos_decimal_2 = PositiveDecimal::try_from(10.111).unwrap();
        let result = pos_decimal_1.checked_add(pos_decimal_2);
        assert!(result.is_err());
    }

    #[test]
    fn test_positive_decimal_checked_sub() {
        let pos_decimal_1 = PositiveDecimal::try_from(21.111).unwrap();
        let pos_decimal_2 = PositiveDecimal::try_from(10.111).unwrap();
        let result = pos_decimal_1.checked_sub(pos_decimal_2);
        assert!(result.is_ok());

        let pos_decimal_1 = PositiveDecimal::try_from(1.1).unwrap();
        let pos_decimal_2 = PositiveDecimal::try_from(10.111).unwrap();
        let result = pos_decimal_1.checked_sub(pos_decimal_2);
        assert!(result.is_err());
    }

    // You could potentially write a macro to reduce the copy+pasted code below
    #[test]
    fn test_tx_try_from_deposit_tx_record() {
        let deposit_amount = Decimal::from_f64(100.002).unwrap();
        let valid_deposit_record = TransactionRecord {
            transaction_type: TransactionRecordType::Deposit,
            client_id: 1,
            transaction_id: 100,
            amount: Some(deposit_amount),
        };

        let valid_deposit = Transaction::try_from(valid_deposit_record);
        assert!(valid_deposit.is_ok());
        assert_eq!(
            valid_deposit.unwrap(),
            Transaction::new(
                1,
                100,
                TransactionType::Deposit {
                    amount: PositiveDecimal::try_from(deposit_amount).unwrap()
                }
            )
        );

        let invalid_deposit_record = TransactionRecord {
            transaction_type: TransactionRecordType::Deposit,
            client_id: 1,
            transaction_id: 100,
            amount: None,
        };

        let invalid_deposit = Transaction::try_from(invalid_deposit_record);
        assert!(invalid_deposit.is_err());
    }

    #[test]
    fn test_tx_try_from_withdrawal_tx_record() {
        let withdrawal_amount = Decimal::from_f64(100.002).unwrap();
        let valid_withdrawal_record = TransactionRecord {
            transaction_type: TransactionRecordType::Withdrawal,
            client_id: 1,
            transaction_id: 100,
            amount: Some(withdrawal_amount),
        };

        let valid_withdrawal = Transaction::try_from(valid_withdrawal_record);
        assert!(valid_withdrawal.is_ok());
        assert_eq!(
            valid_withdrawal.unwrap(),
            Transaction::new(
                1,
                100,
                TransactionType::Withdrawal {
                    amount: PositiveDecimal::try_from(withdrawal_amount).unwrap()
                }
            )
        );

        let invalid_withdrawal_record = TransactionRecord {
            transaction_type: TransactionRecordType::Withdrawal,
            client_id: 1,
            transaction_id: 100,
            amount: None,
        };

        let invalid_withdrawal = Transaction::try_from(invalid_withdrawal_record);
        assert!(invalid_withdrawal.is_err());
    }

    #[test]
    fn test_tx_try_from_dispute_tx_record() {
        let dispute_amount = Decimal::from_f64(100.002).unwrap();
        let valid_dispute_record = TransactionRecord {
            transaction_type: TransactionRecordType::Dispute,
            client_id: 1,
            transaction_id: 100,
            amount: None,
        };

        let valid_dispute = Transaction::try_from(valid_dispute_record);
        assert!(valid_dispute.is_ok());
        assert_eq!(
            valid_dispute.unwrap(),
            Transaction::new(1, 100, TransactionType::Dispute)
        );

        let invalid_dispute_record = TransactionRecord {
            transaction_type: TransactionRecordType::Dispute,
            client_id: 1,
            transaction_id: 100,
            amount: Some(dispute_amount),
        };

        let valid_dispute = Transaction::try_from(invalid_dispute_record);
        assert!(valid_dispute.is_ok());
        assert_eq!(
            valid_dispute.unwrap(),
            Transaction::new(1, 100, TransactionType::Dispute)
        );
    }

    #[test]
    fn test_tx_try_from_resolve_tx_record() {
        let resolve_amount = Decimal::from_f64(100.002).unwrap();
        let valid_resolve_record = TransactionRecord {
            transaction_type: TransactionRecordType::Resolve,
            client_id: 1,
            transaction_id: 100,
            amount: None,
        };

        let valid_resolve = Transaction::try_from(valid_resolve_record);
        assert!(valid_resolve.is_ok());
        assert_eq!(
            valid_resolve.unwrap(),
            Transaction::new(1, 100, TransactionType::Resolve)
        );

        let invalid_resolve_record = TransactionRecord {
            transaction_type: TransactionRecordType::Resolve,
            client_id: 1,
            transaction_id: 100,
            amount: Some(resolve_amount),
        };

        let valid_resolve = Transaction::try_from(invalid_resolve_record);
        assert!(valid_resolve.is_ok());
        assert_eq!(
            valid_resolve.unwrap(),
            Transaction::new(1, 100, TransactionType::Resolve)
        );
    }

    #[test]
    fn test_tx_try_from_chargeback_tx_record() {
        let chargeback_amount = Decimal::from_f64(100.002).unwrap();
        let valid_chargeback_record = TransactionRecord {
            transaction_type: TransactionRecordType::Chargeback,
            client_id: 1,
            transaction_id: 100,
            amount: None,
        };

        let valid_chargeback = Transaction::try_from(valid_chargeback_record);
        assert!(valid_chargeback.is_ok());
        assert_eq!(
            valid_chargeback.unwrap(),
            Transaction::new(1, 100, TransactionType::Chargeback)
        );

        let invalid_chargeback_record = TransactionRecord {
            transaction_type: TransactionRecordType::Chargeback,
            client_id: 1,
            transaction_id: 100,
            amount: Some(chargeback_amount),
        };

        let valid_chargeback = Transaction::try_from(invalid_chargeback_record);
        assert!(valid_chargeback.is_ok());
        assert_eq!(
            valid_chargeback.unwrap(),
            Transaction::new(1, 100, TransactionType::Chargeback)
        );
    }
}
