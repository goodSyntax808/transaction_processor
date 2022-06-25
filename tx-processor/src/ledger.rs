use std::collections::HashMap;

use log::{error, warn};

use crate::account::Account;
use crate::error::TxError;
use crate::transaction::{
    PositiveDecimal, Transact, Transaction, TransactionRecord, TransactionType,
};

#[derive(Debug, Default)]
pub struct Ledger {
    pub(crate) active_accounts: HashMap<u16, Account<false>>,
    pub(crate) locked_accounts: HashMap<u16, Account<true>>,
    pub(crate) transactions: Vec<Transaction>,
    /// Map of `<transaction_id, (client_id, amount)`
    pub(crate) disputed_tx_map: HashMap<u32, (u16, PositiveDecimal)>,
}

impl Ledger {
    pub fn process_transactions(&mut self, transactions: impl IntoIterator<Item = Transaction>) {
        for transaction in transactions {
            self.add_tx(transaction).ok();
        }
    }

    pub fn process_csv_transactions(
        &mut self,
        transactions: impl IntoIterator<Item = Result<TransactionRecord, csv::Error>>,
    ) {
        for transaction in transactions
            .into_iter()
            //.flat_map(|res| res.map_err(|e| error!("Malformed CSV Record: {:?}", e)))
            .flatten()
            .flat_map(|record| {
                Transaction::try_from(record)//.map_err(|e| error!("Malformed Transaction: {:?}", e))
            })
        {
            self.add_tx(transaction)
                //.map_err(|e| warn!("Invalid Transaction: {:?}", e))
                .ok();
        }
    }

    /// # Errors
    /// This function errors if the transaction is on a locked account or if the transaction is
    /// not valid (e.g., a withdrawal greater than the account's balance).
    ///
    /// # Panics
    /// Only if there is an error in the handling of the Chargeback match arm
    pub fn add_tx(&mut self, transaction: Transaction) -> Result<(), TxError> {
        if self.locked_accounts.contains_key(&transaction.client_id) {
            return Err(TxError::LockedAccount);
        }

        let account = self
            .active_accounts
            .entry(transaction.client_id)
            .or_insert_with_key(|&k| Account::new(k));
        match transaction.tx_type {
            TransactionType::Deposit { amount } => {
                account.deposit(amount)?;
            }
            TransactionType::Withdrawal { amount } => {
                account.withdraw(amount)?;
            }
            TransactionType::Dispute => {
                account.dispute(
                    transaction.transaction_id,
                    &self.transactions,
                    &mut self.disputed_tx_map,
                )?;
            }
            TransactionType::Resolve => {
                account.resolve(transaction.transaction_id, &mut self.disputed_tx_map)?;
            }
            TransactionType::Chargeback => {
                let removed_account = self.active_accounts.remove(&transaction.client_id).unwrap();
                let chargeback_res = removed_account
                    .chargeback(transaction.transaction_id, &mut self.disputed_tx_map);
                match chargeback_res {
                    (Ok(locked_account), None) => {
                        self.active_accounts.remove(&locked_account.client_id);
                        self.locked_accounts
                            .insert(locked_account.client_id, locked_account);
                    }
                    (Err(e), Some(removed_account)) => {
                        self.active_accounts
                            .insert(transaction.client_id, removed_account);
                        return Err(e);
                    }
                    (Ok(_), Some(_)) | (Err(_), None) => unreachable!(),
                }
            }
        }
        self.transactions.push(transaction);

        Ok(())
    }

    #[must_use]
    pub fn active_accounts(&self) -> &HashMap<u16, Account<false>> {
        &self.active_accounts
    }

    #[must_use]
    pub fn locked_accounts(&self) -> &HashMap<u16, Account<true>> {
        &self.locked_accounts
    }

    #[must_use]
    pub fn transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rust_decimal::prelude::*;

    #[allow(clippy::too_many_lines)]
    #[test]
    fn test_ledger() {
        let mut ledger = Ledger::default();
        let zero = PositiveDecimal::try_from(Decimal::ZERO).unwrap();
        let amount = PositiveDecimal::try_from(10000.1000).unwrap();
        let client_id = 10;
        let tx_id = 1000;
        let locked_account: Account<true> = Account::<true>::from(Account::new(1));
        ledger.locked_accounts.insert(client_id, locked_account);

        let tx = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        let res = ledger.add_tx(tx);
        assert!(res.is_err());

        let mut ledger = Ledger::default();
        // deposit
        let tx = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        let res = ledger.add_tx(tx);
        assert!(res.is_ok());
        let log = ledger.transactions();
        let tx = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        assert_eq!(log, &vec![tx]);
        let mut account = Account::new(client_id);
        account.deposit(amount).unwrap();
        assert_eq!(ledger.active_accounts().get(&client_id).unwrap(), &account);

        // withdraw
        let smaller_amount = PositiveDecimal::try_from(900.1000).unwrap();
        let tx = Transaction::new(
            client_id,
            tx_id + 1,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let res = ledger.add_tx(tx);
        assert!(res.is_ok());
        let log = ledger.transactions();
        let tx_1 = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        let tx_2 = Transaction::new(
            client_id,
            tx_id + 1,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        assert_eq!(log, &vec![tx_1, tx_2]);
        let mut account = Account::new(client_id);
        account
            .deposit(amount.checked_sub(smaller_amount).unwrap())
            .unwrap();
        assert_eq!(ledger.active_accounts().get(&client_id).unwrap(), &account);

        // dispute
        let tx = Transaction::new(client_id, tx_id + 1, TransactionType::Dispute);
        let res = ledger.add_tx(tx);
        assert!(res.is_ok());
        let log = ledger.transactions();
        let tx_1 = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        let tx_2 = Transaction::new(
            client_id,
            tx_id + 1,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let tx_3 = Transaction::new(client_id, tx_id + 1, TransactionType::Dispute);
        assert_eq!(log, &vec![tx_1, tx_2, tx_3]);
        let balance = &ledger.active_accounts().get(&client_id).unwrap().balance;
        // NOTE demonstation of weird specifications of behavior
        // For a dispute, the instructions say:
        // This means that the clients available funds should decrease by the amount disputed,
        // their held funds should increase by the amount disputed, while their total funds should remain the same.
        //
        // However, if I'm disputing a withdrawal, my available funds should not decrease
        let available = amount
            .checked_sub(smaller_amount)
            .unwrap()
            .checked_sub(smaller_amount)
            .unwrap();
        assert_eq!(balance.available(), &available);
        assert_eq!(balance.held(), &smaller_amount);

        // resolve
        let tx = Transaction::new(client_id, tx_id + 1, TransactionType::Resolve);
        let res = ledger.add_tx(tx);
        assert!(res.is_ok());
        let log = ledger.transactions();
        let tx_1 = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        let tx_2 = Transaction::new(
            client_id,
            tx_id + 1,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let tx_3 = Transaction::new(client_id, tx_id + 1, TransactionType::Dispute);
        let tx_4 = Transaction::new(client_id, tx_id + 1, TransactionType::Resolve);
        assert_eq!(log, &vec![tx_1, tx_2, tx_3, tx_4]);
        let balance = &ledger.active_accounts().get(&client_id).unwrap().balance;
        let available = amount.checked_sub(smaller_amount).unwrap();
        assert_eq!(balance.available(), &available);
        assert_eq!(balance.held(), &zero);

        // withdraw
        let huge_amount = PositiveDecimal::try_from(9_000_000_000.100_0).unwrap();
        let tx = Transaction::new(
            client_id,
            tx_id + 2,
            TransactionType::Withdrawal {
                amount: huge_amount,
            },
        );
        assert_eq!(ledger.transactions().len(), 4);
        let res = ledger.add_tx(tx);
        assert_eq!(ledger.transactions().len(), 4);
        assert!(res.is_err());
        let tx = Transaction::new(
            client_id,
            tx_id + 2,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let res = ledger.add_tx(tx);
        assert!(res.is_ok());
        let log = ledger.transactions();
        let tx_1 = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        let tx_2 = Transaction::new(
            client_id,
            tx_id + 1,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let tx_3 = Transaction::new(client_id, tx_id + 1, TransactionType::Dispute);
        let tx_4 = Transaction::new(client_id, tx_id + 1, TransactionType::Resolve);
        let tx_5 = Transaction::new(
            client_id,
            tx_id + 2,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        assert_eq!(log, &vec![tx_1, tx_2, tx_3, tx_4, tx_5]);
        let balance = &ledger.active_accounts().get(&client_id).unwrap().balance;
        let available = amount
            .checked_sub(smaller_amount)
            .unwrap()
            .checked_sub(smaller_amount)
            .unwrap();
        assert_eq!(balance.available(), &available);
        assert_eq!(balance.held(), &zero);

        // dispute
        let tx = Transaction::new(client_id, tx_id + 2, TransactionType::Dispute);
        let res = ledger.add_tx(tx);
        assert!(res.is_ok());
        let log = ledger.transactions();
        let tx_1 = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        let tx_2 = Transaction::new(
            client_id,
            tx_id + 1,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let tx_3 = Transaction::new(client_id, tx_id + 1, TransactionType::Dispute);
        let tx_4 = Transaction::new(client_id, tx_id + 1, TransactionType::Resolve);
        let tx_5 = Transaction::new(
            client_id,
            tx_id + 2,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let tx_6 = Transaction::new(client_id, tx_id + 2, TransactionType::Dispute);
        assert_eq!(log, &vec![tx_1, tx_2, tx_3, tx_4, tx_5, tx_6]);
        let balance = &ledger.active_accounts().get(&client_id).unwrap().balance;
        let available = amount
            .checked_sub(smaller_amount)
            .unwrap()
            .checked_sub(smaller_amount)
            .unwrap()
            .checked_sub(smaller_amount)
            .unwrap();
        assert_eq!(balance.available(), &available);
        assert_eq!(balance.held(), &smaller_amount);

        // chargeback
        let tx = Transaction::new(client_id, tx_id + 2, TransactionType::Chargeback);
        let res = ledger.add_tx(tx);
        assert!(res.is_ok());
        let log = ledger.transactions();
        let tx_1 = Transaction::new(client_id, tx_id, TransactionType::Deposit { amount });
        let tx_2 = Transaction::new(
            client_id,
            tx_id + 1,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let tx_3 = Transaction::new(client_id, tx_id + 1, TransactionType::Dispute);
        let tx_4 = Transaction::new(client_id, tx_id + 1, TransactionType::Resolve);
        let tx_5 = Transaction::new(
            client_id,
            tx_id + 2,
            TransactionType::Withdrawal {
                amount: smaller_amount,
            },
        );
        let tx_6 = Transaction::new(client_id, tx_id + 2, TransactionType::Dispute);
        let tx_7 = Transaction::new(client_id, tx_id + 2, TransactionType::Chargeback);
        assert_eq!(log, &vec![tx_1, tx_2, tx_3, tx_4, tx_5, tx_6, tx_7]);
        assert!(!ledger.active_accounts().contains_key(&client_id));
        let balance = &ledger.locked_accounts().get(&client_id).unwrap().balance;
        let available = amount
            .checked_sub(smaller_amount)
            .unwrap()
            .checked_sub(smaller_amount)
            .unwrap()
            .checked_sub(smaller_amount)
            .unwrap();
        assert_eq!(balance.available(), &available);
        assert_eq!(balance.held(), &zero);
    }
}
