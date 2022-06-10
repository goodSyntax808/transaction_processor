use std::error::Error;
use std::io;

use clap::Parser;
use csv::{ReaderBuilder, Trim, WriterBuilder};

use tx_processor::ledger::Ledger;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// The input file of transactions
    pub(crate) input_file: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut writer = WriterBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_writer(io::stdout());
    let cli = Cli::parse();

    let mut reader = ReaderBuilder::new()
        .trim(Trim::All)
        .flexible(true)
        .from_path(&cli.input_file)?;
    let mut ledger = Ledger::default();
    ledger.process_csv_transactions(reader.deserialize());

    writer.write_record(&vec!["client", "available", "held", "total", "locked"])?;

    for account in ledger.active_accounts() {
        writer.serialize(account)?;
    }
    for account in ledger.locked_accounts() {
        writer.serialize(account)?;
    }

    Ok(())
}
