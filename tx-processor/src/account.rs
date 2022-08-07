use std::collections::HashMap;
use std::convert::From;

use serde::{ser, ser::SerializeStruct, Serialize, Serializer};

use crate::error::TxError;
use crate::transaction::{PositiveDecimal, Transact, Transaction, TransactionType};

/// The detailing of the amounts available for spending in a client's [Account](crate::account::Account)
/// The total amount of money can be derived by adding the `available` and `held` in this `Balance`
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Balance {
    /// Amount ready for immediate spending
    available: PositiveDecimal,
    /// Amount held by disputed transactions
    held: PositiveDecimal,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Account<const IS_LOCKED: bool> {
    pub(crate) client_id: u16,
    pub(crate) balance: Balance,
}

impl Balance {
    pub(crate) fn available(&self) -> &PositiveDecimal {
        &self.available
    }

    pub(crate) fn held(&self) -> &PositiveDecimal {
        &self.held
    }

    pub(crate) fn total(&self) -> Result<PositiveDecimal, TxError> {
        self.available.checked_add(self.held)
    }
}

impl Serialize for Balance {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Balance", 3)?;
        state.serialize_field("available", &self.available)?;
        state.serialize_field("held", &self.held)?;
        let total = self.total().map_err(|_| {
            ser::Error::custom("Balances were too high, unable to serialize correct data")
        })?;
        state.serialize_field("total", &total)?;
        state.end()
    }
}

impl From<Account<false>> for Account<true> {
    fn from(account: Account<false>) -> Self {
        Account {
            client_id: account.client_id,
            balance: account.balance,
        }
    }
}

impl Account<false> {
    #[must_use]
    pub fn new(client_id: u16) -> Self {
        Account {
            client_id,
            balance: Balance::default(),
        }
    }
}

impl Transact for Account<false> {
    fn deposit(&mut self, amount: PositiveDecimal) -> Result<(), TxError> {
        self.balance.available = self.balance.available.checked_add(amount)?;
        Ok(())
    }

    fn withdraw(&mut self, amount: PositiveDecimal) -> Result<(), TxError> {
        self.balance.available = self.balance.available.checked_sub(amount)?;
        Ok(())
    }

    /// Assumption: the `transaction_log` **must** be ordered chronologically
    fn dispute(
        &mut self,
        disputed_tx_id: u32,
        transaction_log: &[Transaction],
        disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> Result<(), TxError> {
        if disputed_tx_map.contains_key(&disputed_tx_id) {
            return Err(TxError::BadDispute);
        }

        if let Some(disputed_transaction) = transaction_log
            .iter()
            .find(|&t| t.transaction_id == disputed_tx_id)
        {
            if self.client_id != disputed_transaction.client_id {
                return Err(TxError::InsufficientPermission);
            }

            match disputed_transaction.tx_type {
                TransactionType::Deposit { amount } | TransactionType::Withdrawal { amount } => {
                    self.balance.available = self.balance.available.checked_sub(amount)?;
                    self.balance.held = self.balance.held.checked_add(amount)?;
                    disputed_tx_map.insert(disputed_tx_id, (self.client_id, amount));
                    Ok(())
                }
                _ => Err(TxError::BadDispute),
            }
        } else {
            Err(TxError::NotFound)
        }
    }

    fn resolve(
        &mut self,
        transaction_id: u32,
        disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> Result<(), TxError> {
        match disputed_tx_map.get(&transaction_id) {
            Some(&(client_id, amount)) => {
                if self.client_id == client_id {
                    self.balance.available = self.balance.available.checked_add(amount)?;
                    self.balance.held = self.balance.held.checked_sub(amount)?;
                    disputed_tx_map.remove(&transaction_id);
                    Ok(())
                } else {
                    Err(TxError::InsufficientPermission)
                }
            }
            None => Err(TxError::NotFound),
        }
    }

    fn chargeback(
        mut self,
        transaction_id: u32,
        disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> (Result<Account<true>, TxError>, Option<Account<false>>) {
        match disputed_tx_map.get(&transaction_id) {
            Some(&(client_id, amount)) => {
                if client_id == self.client_id {
                    let held_sub_res = self.balance.held.checked_sub(amount);
                    match held_sub_res {
                        Ok(amount) => {
                            self.balance.held = amount;
                            disputed_tx_map.remove(&transaction_id);
                            (Ok(Account::<true>::from(self)), None)
                        }
                        Err(e) => (Err(e), Some(self)),
                    }
                } else {
                    (Err(TxError::InsufficientPermission), Some(self))
                }
            }
            None => (Err(TxError::NotFound), Some(self)),
        }
    }
}

impl Transact for Account<true> {
    fn deposit(&mut self, _amount: PositiveDecimal) -> Result<(), TxError> {
        Err(TxError::LockedAccount)
    }

    fn withdraw(&mut self, _amount: PositiveDecimal) -> Result<(), TxError> {
        Err(TxError::LockedAccount)
    }

    fn dispute(
        &mut self,
        _disputed_tx_id: u32,
        _transaction_log: &[Transaction],
        _disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> Result<(), TxError> {
        Err(TxError::LockedAccount)
    }

    fn resolve(
        &mut self,
        _transaction_id: u32,
        _disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> Result<(), TxError> {
        Err(TxError::LockedAccount)
    }

    fn chargeback(
        self,
        _transaction_id: u32,
        _disputed_tx_map: &mut HashMap<u32, (u16, PositiveDecimal)>,
    ) -> (Result<Account<true>, TxError>, Option<Account<false>>) {
        (Err(TxError::LockedAccount), None)
    }
}

impl<const IS_LOCKED: bool> Serialize for Account<IS_LOCKED> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Account", 5)?;
        state.serialize_field("client", &self.client_id)?;
        state.serialize_field("available", &self.balance.available())?;
        state.serialize_field("held", &self.balance.held())?;
        state.serialize_field(
            "total",
            &self
                .balance
                .total()
                .map_err(|_| serde::ser::Error::custom("Overflowed balance total"))?,
        )?;
        state.serialize_field("locked", &IS_LOCKED)?;
        state.end()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rust_decimal::prelude::*;

    #[test]
    fn test_transact_locked_account() {
        let mut locked_account: Account<true> = Account::<true>::from(Account::new(1));
        let amount = PositiveDecimal::try_from(42.2222).unwrap();
        assert!(locked_account.deposit(amount).is_err());
        assert!(locked_account.withdraw(amount).is_err());
        assert!(locked_account
            .dispute(888, &[], &mut HashMap::new())
            .is_err());
        assert!(locked_account.resolve(888, &mut HashMap::new()).is_err());
        assert!(locked_account
            .chargeback(888, &mut HashMap::new())
            .0
            .is_err());
        let locked_account: Account<true> = Account::<true>::from(Account::new(1));
        assert!(locked_account
            .chargeback(888, &mut HashMap::new())
            .1
            .is_none());
    }

    #[test]
    fn test_deposit_unlocked_account() {
        let mut account = Account::new(1);
        let zero = PositiveDecimal::try_from(Decimal::ZERO).unwrap();
        let amount = PositiveDecimal::try_from(42.2222).unwrap();
        assert_eq!(account.balance.available, zero);
        assert_eq!(account.balance.held, zero);
        let res = account.deposit(amount);
        assert!(res.is_ok());
        assert_eq!(account.balance.available, amount);
        assert_eq!(account.balance.held, zero);
    }

    #[test]
    fn test_withdraw_unlocked_account() {
        // setup
        let mut account = Account::new(1);
        let zero = PositiveDecimal::try_from(Decimal::ZERO).unwrap();
        let amount = PositiveDecimal::try_from(42.2222).unwrap();
        let res = account.deposit(amount);
        assert!(res.is_ok());
        assert_eq!(account.balance.available, amount);
        assert_eq!(account.balance.held, zero);

        // perform valid withdrawal
        let withdrawal_amount = PositiveDecimal::try_from(1.2222).unwrap();
        let new_amount = PositiveDecimal::try_from(41.0000).unwrap();
        let res = account.withdraw(withdrawal_amount);
        assert!(res.is_ok());
        assert_eq!(account.balance.available, new_amount);
        assert_eq!(account.balance.held, zero);

        // perform invalid withdrawal
        let withdrawal_amount = PositiveDecimal::try_from(45.0).unwrap();
        let res = account.withdraw(withdrawal_amount);
        assert!(res.is_err());
        // balance and held should not have changed
        assert_eq!(account.balance.available, new_amount);
        assert_eq!(account.balance.held, zero);

        // full withdrawal
        let res = account.withdraw(new_amount);
        assert!(res.is_ok());
        assert_eq!(account.balance.available, zero);
        assert_eq!(account.balance.held, zero);
    }

    #[test]
    fn test_dispute_unlocked_account() {
        // setup
        let disputed_tx_id: u32 = 999;
        let client_id: u16 = 5;
        let zero = PositiveDecimal::try_from(Decimal::ZERO).unwrap();
        let amount = PositiveDecimal::try_from(10000.1000).unwrap();
        let mut map = HashMap::new();
        map.insert(disputed_tx_id, (client_id, zero));

        // can't dispute something that's already disputed
        let mut account = Account::new(client_id);
        let res = account.dispute(disputed_tx_id, &[], &mut map);
        assert!(res.is_err());

        // can't find a transaction
        map.clear();
        let res = account.dispute(disputed_tx_id, &[], &mut map);
        assert!(res.is_err());

        // can't dispute a transaction from someone else
        let tx = Transaction::new(
            client_id + 1,
            disputed_tx_id,
            TransactionType::Deposit { amount },
        );
        let res = account.dispute(disputed_tx_id, &[tx], &mut map);
        assert!(res.is_err());

        // can't dispute a transaction other than a deposit or withdrawal
        let tx = Transaction::new(client_id, disputed_tx_id, TransactionType::Dispute);
        let res = account.dispute(disputed_tx_id, &[tx], &mut map);
        assert!(res.is_err());
        let tx = Transaction::new(client_id, disputed_tx_id, TransactionType::Resolve);
        let res = account.dispute(disputed_tx_id, &[tx], &mut map);
        assert!(res.is_err());
        let tx = Transaction::new(client_id, disputed_tx_id, TransactionType::Chargeback);
        let res = account.dispute(disputed_tx_id, &[tx], &mut map);
        assert!(res.is_err());

        // cant dispute deposits or withdrawals without funds
        let tx = Transaction::new(
            client_id,
            disputed_tx_id,
            TransactionType::Deposit { amount },
        );
        assert!(map.is_empty());
        let res = account.dispute(disputed_tx_id, &[tx], &mut map);
        assert!(res.is_err());
        assert!(map.is_empty());

        let tx = Transaction::new(
            client_id,
            disputed_tx_id,
            TransactionType::Withdrawal { amount },
        );
        assert!(map.is_empty());
        let res = account.dispute(disputed_tx_id, &[tx], &mut map);
        assert!(res.is_err());
        assert!(map.is_empty());

        // can dispute deposits and withdrawals with funds
        let large_amount = PositiveDecimal::try_from(100_000_000.100_0).unwrap();
        account.deposit(large_amount).unwrap();
        assert_eq!(account.balance.available, large_amount);
        assert_eq!(account.balance.held, zero);
        let tx_1 = Transaction::new(
            client_id,
            disputed_tx_id - 1,
            TransactionType::Deposit {
                amount: large_amount,
            },
        );
        let tx_2 = Transaction::new(
            client_id,
            disputed_tx_id,
            TransactionType::Deposit { amount },
        );
        assert!(map.is_empty());
        let res = account.dispute(disputed_tx_id, &[tx_1, tx_2], &mut map);
        assert!(res.is_ok());
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&disputed_tx_id).unwrap(), &(client_id, amount));
        assert_eq!(
            account.balance.available,
            large_amount.checked_sub(amount).unwrap()
        );
        assert_eq!(account.balance.held, amount);

        let tx_1 = Transaction::new(
            client_id,
            disputed_tx_id - 1,
            TransactionType::Deposit {
                amount: large_amount,
            },
        );
        let tx_2 = Transaction::new(
            client_id,
            disputed_tx_id,
            TransactionType::Withdrawal { amount },
        );
        map.clear();
        assert!(map.is_empty());
        let res = account.dispute(disputed_tx_id, &[tx_1, tx_2], &mut map);
        assert!(res.is_ok());
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&disputed_tx_id).unwrap(), &(client_id, amount));
        assert_eq!(
            account.balance.available,
            large_amount
                .checked_sub(amount)
                .unwrap()
                .checked_sub(amount)
                .unwrap()
        );
        assert_eq!(account.balance.held, amount.checked_add(amount).unwrap());
    }

    #[test]
    fn test_resolve_unlocked_account() {
        // setup
        let disputed_tx_id: u32 = 999;
        let client_id: u16 = 5;
        let zero = PositiveDecimal::try_from(Decimal::ZERO).unwrap();
        let amount = PositiveDecimal::try_from(10000.1000).unwrap();
        let mut map = HashMap::new();

        // can't resolve something that's not in the map
        let mut account = Account::new(client_id);
        let res = account.resolve(disputed_tx_id, &mut map);
        assert!(res.is_err());

        // can't resolve something for a different client_id
        map.insert(disputed_tx_id, (client_id + 1, amount));
        assert_eq!(map.len(), 1);
        let res = account.resolve(disputed_tx_id, &mut map);
        assert!(res.is_err());
        assert_eq!(map.len(), 1);

        // can resolve something valid
        map.clear();
        assert!(map.is_empty());
        assert_eq!(account.balance.available, zero);
        assert_eq!(account.balance.held, zero);
        account.deposit(amount).unwrap();
        assert_eq!(account.balance.available, amount);
        assert_eq!(account.balance.held, zero);
        let tx = Transaction::new(
            client_id,
            disputed_tx_id,
            TransactionType::Deposit { amount },
        );
        account.dispute(disputed_tx_id, &[tx], &mut map).unwrap();
        assert_eq!(account.balance.available, zero);
        assert_eq!(account.balance.held, amount);
        let res = account.resolve(disputed_tx_id, &mut map);
        assert!(res.is_ok());
        assert_eq!(account.balance.available, amount);
        assert_eq!(account.balance.held, zero);
    }

    #[test]
    fn test_chargeback_unlocked_account() {
        // setup
        let disputed_tx_id: u32 = 999;
        let client_id: u16 = 5;
        let zero = PositiveDecimal::try_from(Decimal::ZERO).unwrap();
        let amount = PositiveDecimal::try_from(10000.1000).unwrap();
        let mut map = HashMap::new();

        // can't chargeback something that's not in the map
        let account = Account::new(client_id);
        let (res, opt) = account.chargeback(disputed_tx_id, &mut map);
        assert!(res.is_err());
        let account = opt.unwrap();

        // can't chargeback something for a different client_id
        map.insert(disputed_tx_id, (client_id + 1, amount));
        assert_eq!(map.len(), 1);
        let (res, opt) = account.chargeback(disputed_tx_id, &mut map);
        assert!(res.is_err());
        assert_eq!(map.len(), 1);

        // can chargeback something valid
        map.clear();
        assert!(map.is_empty());
        let mut account = opt.unwrap();
        assert_eq!(account.balance.available, zero);
        assert_eq!(account.balance.held, zero);
        account.deposit(amount).unwrap();
        assert_eq!(account.balance.available, amount);
        assert_eq!(account.balance.held, zero);
        let tx = Transaction::new(
            client_id,
            disputed_tx_id,
            TransactionType::Deposit { amount },
        );
        account.dispute(disputed_tx_id, &[tx], &mut map).unwrap();
        assert_eq!(account.balance.available, zero);
        assert_eq!(account.balance.held, amount);
        let (res, opt) = account.chargeback(disputed_tx_id, &mut map);
        assert!(res.is_ok());
        assert!(opt.is_none());
        let locked_account = res.unwrap();
        assert_eq!(locked_account.balance.available, zero);
        assert_eq!(locked_account.balance.held, zero);
    }
}
