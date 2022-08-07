#![deny(rust_2018_idioms)]
#![deny(clippy::correctness)]
#![deny(clippy::perf)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod account;
pub mod error;
pub mod ledger;
pub mod transaction;
