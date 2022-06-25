//! Generates a test file with random data in 8 columns 2 of each type.
//! can be run with `cargo run --bin generate`

use rand::{thread_rng, Rng};
use tx_processor::transaction::TransactionRecord;


fn main() {
    let mut writer = csv::WriterBuilder::new().from_path("resources/input/rand.csv").unwrap();
    let mut rng = thread_rng();
    for _ in 0..100_000 {
        let t: TransactionRecord = rng.gen();
        writer.serialize(t).unwrap();
    }
    writer.flush().unwrap();
}