# turbosql

## Turbosql: Easy Data Persistence Layer, backed by SQLite

WORK IN PROGRESS, use at your own risk. :)

Macros for easily persisting Rust `struct`s to an on-disk SQLite database and later retrieving them, optionally based on your own predicates.

```rust
use turbosql::Turbosql;

#[derive(Turbosql)]
struct Person {
 rowid: Option<i64>,  // rowid member required & enforced at compile time
 name: String,
 age: Option<i64>,
 image_jpg: Option<Blob>
}

fn main() {}
```

### Design Goals

- API with minimal cognitive complexity and boilerplate
- High performance
- Reliable storage
- Surface the power of SQL — make simple things easy, and complex things possible
- In the spirit of Rust, move as many errors as possible to compile time

## Examples

<table>

<tr><td><b>Primitive type</b></td><td><br>

```rust
let result = select!(String "SELECT name FROM person")?;
```

Returns one value cast to specified type, returns `TurboSql::Error::QueryReturnedNoRows` if no rows available.

```rust
let result = select!(String "name FROM person WHERE rowid = ?", rowid)?;
```

`SELECT` keyword is **always optional** when using `select!`; it's added automatically as needed.<br>Parameter binding is straightforward.

</td></tr>

<tr><td><b>Tuple</b></td><td><br>

```rust
let result = select!((String, i64) "name, age FROM person")?;
```

Use tuple types for multiple manually declared columns.

</td></tr>

<tr><td><b>Anonymous struct</b></td><td><br>

```rust
let result = select!("name_String, age_i64 FROM person")?;
println!("{}", result.name);
```

Types must be specified in column names to generate an anonymous struct.

</td></tr>

<tr><td><b><code>Vec&lt;_&gt;</code></b></td><td><br>

```rust
let result = select!(Vec<String> "name FROM person")?;
```

Returns `Vec` of another type. If no rows, returns empty `Vec`. (Tuple types work inside, as well.)

```rust
let result = select!(Vec<_> "name_String, age_i64 FROM person")?;
```

Anonymous structs work, too.

</td></tr>

<tr><td><b><code>Option&lt;_&gt;</code></b></td><td><br>

```rust
let result = select!(Option<String> "name FROM person")?;
```

Returns `Ok(None)` if no rows, `Error(Turbosql::Error)` on error.

</td></tr>

<tr><td><b>Your struct</b></td><td><br>

```rust
let result = select!(Person "WHERE name = ?", name)?;
```

Column list and table name are optional if type is a `#[derive(Turbosql)]` struct.

```rust
let result = select!(Vec<NameAndAdult> "name, age >= 18 AS adult FROM person")?;
```

You can use other struct types as well; column names must match the struct.<br>Implement `Default` to avoid specifying unused column names.<br>(And, of course, you can put it all in a `Vec` or `Option` as well.)

```rust
let result = select!(Vec<Person>)?;
```

Sometimes everything is optional; this example will retrieve all `Person` rows.

</td></tr>

<tr><td><b>Transactions</b></td><td><br>

```rust
transaction! {
  if select!(Option<Person> "WHERE name = ?", name)?.is_none() {
    Person { ... }.insert!()?;
  }
}
```

- How does this work with threads and async?
- What if the transaction fails to commit?
- Nested transactions not supported.
- Calling other functions in a transaction? Async? This gets messy. Just say that any Turbosql calls outside of the literal text `transaction!{}` body will work fine, but _not_ be part of the transaction?

Inititally, this implementation will just open a new SQLite connection, and use it for all child calls.

</td></tr>

</table>
<br>

### License: MIT OR Apache-2.0
