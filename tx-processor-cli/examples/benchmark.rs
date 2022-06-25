use std::error::Error;
use std::io;
use std::time::Instant;

use log::{warn, error};
use csv::{ReaderBuilder, Trim, WriterBuilder};

use tx_processor::ledger::Ledger;


fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut writer = WriterBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_writer(io::stdout());

    let mut reader = ReaderBuilder::new()
        .trim(Trim::All)
        .flexible(true)
        .from_path(&"resources/input/rand.csv")?;
    let mut ledger = Ledger::default();
    let start = Instant::now();
    ledger.process_csv_transactions(reader.deserialize());
    let elapsed = start.elapsed();
    error!("Processing took: {:.2?}", elapsed);

    writer.write_record(&vec!["client", "available", "held", "total", "locked"])?;
    let start_writing = Instant::now();

    for account in ledger.active_accounts() {
        writer.serialize(account)?;
    }
    for account in ledger.locked_accounts() {
        writer.serialize(account)?;
    }
    let elapsed_writing = start_writing.elapsed();
    warn!("Writing took: {:.2?}", elapsed_writing);


    warn!("Total took: {:.2?}", start.elapsed());

    Ok(())
}
