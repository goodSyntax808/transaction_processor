use csv::{ReaderBuilder, Trim};
use tx_processor::ledger::Ledger;
use tx_processor::transaction::{PositiveDecimal, Transaction, TransactionType};

fn make_simple_tx() -> Vec<Transaction> {
    let amount_1 = PositiveDecimal::try_from(1.0000).unwrap();
    let tx_1 = Transaction::new(1, 1, TransactionType::Deposit { amount: amount_1 });
    let amount_2 = PositiveDecimal::try_from(2.0000).unwrap();
    let tx_2 = Transaction::new(2, 2, TransactionType::Deposit { amount: amount_2 });
    let tx_3 = Transaction::new(1, 3, TransactionType::Deposit { amount: amount_2 });
    let amount_3 = PositiveDecimal::try_from(1.5000).unwrap();
    let tx_4 = Transaction::new(1, 4, TransactionType::Withdrawal { amount: amount_3 });
    let txs = vec![tx_1, tx_2, tx_3, tx_4];

    txs
}

#[test]
fn test_simple_transactions() {
    let mut reader = ReaderBuilder::new()
        .trim(Trim::All)
        .from_path(&"../resources/input/tx-input1.csv")
        .unwrap();
    let mut ledger = Ledger::default();
    ledger.process_csv_transactions(reader.deserialize());

    let txs = make_simple_tx();
    assert_eq!(ledger.transactions(), &txs);
}

#[test]
fn test_invalid_record() {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(Trim::All)
        .from_path(&"../resources/input/bad-record.csv")
        .unwrap();
    let mut ledger = Ledger::default();
    ledger.process_csv_transactions(reader.deserialize());
    assert_eq!(ledger.transactions().len(), 3);
}

#[test]
fn test_invalid_tx_struct() {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(Trim::All)
        .from_path(&"../resources/input/invalid-transaction.csv")
        .unwrap();
    let mut ledger = Ledger::default();
    ledger.process_csv_transactions(reader.deserialize());
    assert_eq!(ledger.transactions().len(), 3);
}

#[test]
fn test_resolve() {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(Trim::All)
        .from_path(&"../resources/input/resolve.csv")
        .unwrap();
    let mut ledger = Ledger::default();
    ledger.process_csv_transactions(reader.deserialize());

    let mut txs = make_simple_tx();
    let amount_1 = PositiveDecimal::try_from(2000.0000).unwrap();
    let amount_2 = PositiveDecimal::try_from(10.0000).unwrap();
    let tx_1 = Transaction::new(3, 6, TransactionType::Deposit { amount: amount_1 });
    let tx_2 = Transaction::new(3, 7, TransactionType::Withdrawal { amount: amount_2 });
    let tx_3 = Transaction::new(3, 7, TransactionType::Dispute);
    let tx_4 = Transaction::new(3, 7, TransactionType::Resolve);
    txs.push(tx_1);
    txs.push(tx_2);
    txs.push(tx_3);
    txs.push(tx_4);
    assert_eq!(ledger.transactions(), &txs);
    assert_eq!(ledger.active_accounts().len(), 3);
    assert_eq!(ledger.locked_accounts().len(), 0);
}

#[test]
fn test_chargeback() {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(Trim::All)
        .from_path(&"../resources/input/chargeback.csv")
        .unwrap();
    let mut ledger = Ledger::default();
    ledger.process_csv_transactions(reader.deserialize());

    let mut txs = make_simple_tx();

    let amount_1 = PositiveDecimal::try_from(2000.0000).unwrap();
    let amount_2 = PositiveDecimal::try_from(10.0000).unwrap();
    let tx_1 = Transaction::new(3, 6, TransactionType::Deposit { amount: amount_1 });
    let tx_2 = Transaction::new(3, 7, TransactionType::Withdrawal { amount: amount_2 });
    let tx_3 = Transaction::new(3, 7, TransactionType::Dispute);
    let tx_4 = Transaction::new(3, 7, TransactionType::Chargeback);
    txs.push(tx_1);
    txs.push(tx_2);
    txs.push(tx_3);
    txs.push(tx_4);

    assert_eq!(ledger.transactions(), &txs);
    assert_eq!(ledger.active_accounts().len(), 2);
    assert_eq!(ledger.locked_accounts().len(), 1);
}
