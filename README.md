### Intro
A simple payment processing engine.


Note that I did spent over the requested amount of time on this because I found
it to be a fun challenge and I am trying to get back into writing Rust more 
since I don't currently use it at my day job.

### Usage

```
cargo run -- resources/input/tx-input1.csv > accounts.csv
```


----
### Design
One of my favorite things about Rust and something I tried to introduce to the
design is using type information to guarantee correctness of a program. Here
are some notes about how I tried to accomplish that.

With that mindset, only valid transactions are written to a `Ledger`'s
`transactions: Vec<Transaction>`

#### Newtype pattern: `PositiveDecimal`
The newtype idiom gives compile time guarantees that the right type of value is
supplied to a program. By only allowing positive values, it simplifies processing
of transactions and making sure a customer's balance can't go negative. By
only implementing `checked_add` and `checked_sub` for this newtype, we know
that we will have a valid type after adding or subtracting another `PositiveDecimal`.

#### Typestate pattern: `const` generics and `Account<const IS_LOCKED: BOOL>`
Once an account has a chargeback, it is locked (and currently there is no
specification for how an account is unlocked, though adding this feature is possible).

If an account is locked, it cannot have any other transactions on it. The simple way
to check this is by storing an `is_locked: bool` field in the `Account` struct and
check the value at runtime when trying to perform a transaction on the account.
This is brittle since this check can easily be forgotten and also it costs a 
a (very) small amount of performance+memory for this runtime check.

Instead of doing a runtime check, the `Account` struct is paramterized by a const
boolean value to indicate if it's locked. This is actually my first time using
const generics but it was very nice to use.


#### Parse, Don't Validate, and `TransactionRecord`, `Transaction`, and `TryFrom`
In order to protect against bad input from the CSV file/user input, I created two structs,
`TransactionRecord` and `Transaction`, and implemented `TryFrom<TransactionRecord> for Transaction`
which would perform the checks to guarantee that a `Transaction` is well formed. E.g.,
a `Dispute` transaction must not have an amount but a `Deposit` or `Withdrawal` must.


----
### Notes && Possible Improvements
- Use fuzzing for testing 
- Add cacheing of the `transaction_log` that is scanned when a dispute is made.  Use a LRU map, since recently made transactions are most likely to be disputed.
- Documentation
- Add async
- Improve `TxError` beyond a simple enum

