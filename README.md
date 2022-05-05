# Turbosql

[<img alt="github" src="https://img.shields.io/badge/github-trevyn/turbosql-663399?style=for-the-badge&labelColor=555555&logo=github" height="27">](https://github.com/trevyn/turbosql)
[<img alt="crates.io" src="https://img.shields.io/crates/v/turbosql.svg?style=for-the-badge&color=ffc833&logo=rust" height="27">](https://crates.io/crates/turbosql)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-turbosql-353535?style=for-the-badge&labelColor=555555&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="27">](https://docs.rs/turbosql)

An easy local data persistence layer, backed by SQLite.

- Schema auto-defined by your Rust `struct`s
- Automatic schema migrations
- Super-simple basic `INSERT`/`SELECT`/`UPDATE`/`DELETE` operations
- Use complex SQL if that's your jam
- Validates all SQL (including user-supplied) at compile time

## Usage

```rust
use turbosql::{Turbosql, select, execute};

#[derive(Turbosql, Default)]
struct Person {
    rowid: Option<i64>, // rowid member required & enforced at compile time
    name: Option<String>,
    age: Option<i64>,
    image_jpg: Option<Vec<u8>>
}

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let name = "Joe";

    // INSERT a row
    let rowid = Person {
        name: Some(name.to_string()),
        age: Some(42),
        ..Default::default()
    }.insert()?;

    // SELECT all rows
    let people = select!(Vec<Person>)?;

    // SELECT multiple rows with a predicate
    let people = select!(Vec<Person> "WHERE age > " 21)?;

    // SELECT a single row with a predicate
    let mut person = select!(Person "WHERE name = " name)?;

    // UPDATE based on rowid, rewrites all fields in database row
    person.age = Some(43);
    person.update()?;

    // UPDATE with manual SQL
    execute!("UPDATE person SET age = " 44 " WHERE name = " name)?;

    // DELETE
    execute!("DELETE FROM person WHERE rowid = " 1)?;

    Ok(())
}
```

See [`integration_test.rs`](https://github.com/trevyn/turbosql/blob/main/turbosql/tests/integration_test.rs) or [trevyn/turbo](https://github.com/trevyn/turbo-also-historical/blob/main/turbo_server/src/schema.rs) for more usage examples!

## Under the Hood

Turbosql generates a SQLite schema and prepared queries for each struct:

```rust
use turbosql::{Turbosql, Blob};

#[derive(Turbosql, Default)]
struct Person {
    rowid: Option<i64>, // rowid member required & enforced
    name: Option<String>,
    age: Option<i64>,
    image_jpg: Option<Blob>
}
```

&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;↓&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;auto-generates and validates the schema

```sql
CREATE TABLE person (
    rowid INTEGER PRIMARY KEY,
    name TEXT,
    age INTEGER,
    image_jpg BLOB,
) STRICT

INSERT INTO person (rowid, name, age, image_jpg) VALUES (?, ?, ?, ?)

SELECT rowid, name, age, image_jpg FROM person
```

Queries with SQL predicates are also assembled and validated at compile time. Note that SQL types vs Rust types for parameter bindings are not currently checked at compile time.

```rust,ignore
let people = select!(Vec<Person> "WHERE age > ?", 21);
```

&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;↓

```sql
SELECT rowid, name, age, image_jpg FROM person WHERE age > ?
```

## Automatic Schema Migrations

At compile time, the `#[derive(Turbosql)]` macro runs and creates a `migrations.toml` file in your project root that describes the database schema.

Each time you change a `struct` declaration and the macro is re-run (e.g. by `cargo` or `rust-analyzer`), migration SQL statements are generated that update the database schema. These new statements are recorded in `migrations.toml`, and are automatically embedded in your binary.

```rust
#[derive(turbosql::Turbosql, Default)]
struct Person {
    rowid: Option<i64>,
    name: Option<String>
}
```

&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;↓&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;auto-generates `migrations.toml`

```toml
migrations_append_only = [
  'CREATE TABLE person(rowid INTEGER PRIMARY KEY) STRICT',
  'ALTER TABLE person ADD COLUMN name TEXT',
]
output_generated_schema_for_your_information_do_not_edit = '''
  CREATE TABLE person (
    rowid INTEGER PRIMARY KEY,
    name TEXT
  ) STRICT
'''
```

When your schema changes, any new version of your binary will automatically migrate any older database file to the current schema by applying the appropriate migrations in sequence.

This migration process is a one-way ratchet: Old versions of the binary run on a database file with a newer schema will detect a schema mismatch and will be blocked from operating on the futuristically-schema'd database file.

Unused or reverted migrations that are created during development can be manually removed from `migrations.toml` before being released, but any database files that have already applied these deleted migrations will error and must be rebuilt. Proceed with care. When in doubt, refrain from manually editing `migrations.toml`, and everything should work fine.

- Just declare and freely append fields to your `struct`s.
- Check out the `migrations.toml` file that is generated in your project root to see what's happening.
- If you run into any weird compiler errors, try just re-compiling first; depending on the order the proc macros run, sometimes it just needs a little push to get in sync after a schema change.
- Schema migrations are one-way, append-only. This is similar to the approach taken by [leafac/sqlite-migration](https://github.com/leafac/sqlite-migration#no-down-migrations) for the Node.js ecosystem; see that project for a discussion of the advantages!
- On launch, versions of your binary built with a newer schema will automatically apply the appropriate migrations to an older database.
- If you're feeling adventurous, you can add your own schema migration entries to the bottom of the list. (For creating indexes, etc.)
- You can hand-write complex migrations as well, see [turbo/migrations.toml](https://github.com/trevyn/turbo-also-historical/blob/main/migrations.toml) for some examples.
- Please open a GitHub issue with any questions or suggestions!

## Where's my data?

The SQLite database file is created in the directory returned by [`directories_next::ProjectDirs::data_dir()`](https://docs.rs/directories-next/%5E2.0.0/directories_next/struct.ProjectDirs.html#method.data_dir) + your executable's filename stem, which resolves to something like:

<table><tr><td>Linux</td><td><br>

`$XDG_DATA_HOME`/`{exe_name}` or `$HOME`/.local/share/`{exe_name}` _/home/alice/.local/share/fooapp/fooapp.sqlite_

</td></tr><tr><td>macOS</td><td><br>

`$HOME`/Library/Application&nbsp;Support/`{exe_name}` _/Users/Alice/Library/Application&nbsp;Support/org.fooapp.fooapp/fooapp.sqlite_

</td></tr><tr><td>Windows</td><td><br>

`{FOLDERID_LocalAppData}`\\`{exe_name}`\\data _C:\Users\Alice\AppData\Local\fooapp\fooapp\data\fooapp.sqlite_

</td></tr></table>

## Transactions and `async`

SQLite, and indeed many filesystems in general, only provide blocking (synchronous) APIs. The correct approach when using blocking APIs in a Rust `async` ecosystem is to use your executor's facility for running a closure on a thread pool in which blocking is expected. For example:

```rust
#[derive(turbosql::Turbosql, Default)]
struct Person {
    rowid: Option<i64>,
    name: Option<String>
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let person = tokio::task::spawn_blocking(|| {
        turbosql::select!(Option<Person> "WHERE name = ?", "Joe")
    }).await??;
    Ok(())
}
```

(Note that `spawn_blocking` returns a `JoinHandle` that must itself be unwrapped, hence the need for `??` near the end of these examples.)

Under the hood, Turbosql uses persistent [`thread_local`](https://doc.rust-lang.org/std/macro.thread_local.html) database connections, so a continuous sequence of database calls from the same thread are guaranteed to use the same exclusive database connection. Thus, `async` transactions can be performed as such:

```rust
use turbosql::{Turbosql, select, execute};

#[derive(Turbosql, Default)]
struct Person {
    rowid: Option<i64>,
    age: Option<i64>
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::task::spawn_blocking(|| -> Result<(), turbosql::Error> {
        Person { rowid: None, age: Some(21) }.insert()?;
        execute!("BEGIN IMMEDIATE TRANSACTION")?;
        let p = select!(Person "WHERE rowid = ?", 1)?;
        // [ ...do any other blocking things... ]
        execute!(
            "UPDATE person SET age = ? WHERE rowid = ?",
            p.age.unwrap_or_default() + 1,
            1
        )?;
        execute!("COMMIT")?;
        Ok(())
    }).await??;
    Ok(())
}
```

Turbosql sets a SQLite [`busy_timeout`](https://sqlite.org/c3ref/busy_timeout.html) of 3 seconds, so any table lock contention is automatically re-tried up to that duration, after which the command that was unable to acquire a lock will return with an error.

For further discussion of Turbosql's approach to `async` and transactions, see [https://github.com/trevyn/turbosql/issues/4](https://github.com/trevyn/turbosql/issues/4). Ideas for improvements to the ergonomics of the solution are very welcome.

## `-wal` and `-shm` files

SQLite is an extremely reliable database engine, but it helps to understand how it interfaces with the filesystem. The main `.sqlite` file contains the bulk of the database. During database writes, SQLite also creates `.sqlite-wal` and `.sqlite-shm` files. If the host process is terminated without flushing writes, you may end up with these three files when you expected to have a single file. This is always fine; on next launch, SQLite knows how to resolve any interrupted writes and make sense of the world. However, if the `-wal` and/or `-shm` files are present, they **must be considered essential to database integrity**. Deleting them may result in a corrupted database. See [https://sqlite.org/tempfiles.html](https://sqlite.org/tempfiles.html).

## Example Query Forms

Check [`integration_test.rs`](https://github.com/trevyn/turbosql/blob/main/turbosql/tests/integration_test.rs) for more examples of what works and is tested in CI.

<table>

<tr><td><b>&nbsp;Primitive&nbsp;type</b></td><td><br>

```rust,ignore
let result = select!(String "SELECT name FROM person")?;
```

Returns one value cast to specified type, returns `Error` if no rows available.

```rust,ignore
let result = select!(String "name FROM person WHERE rowid = ?", rowid)?;
```

`SELECT` keyword is **always optional** when using `select!`; it's added automatically as needed.<br>Parameter binding is straightforward.

</td></tr>

<tr><td>&nbsp;<b><code>Vec&lt;_&gt;</code></b></td><td><br>

```rust,ignore
let result = select!(Vec<String> "name FROM person")?;
```

Returns `Vec` containing another type. If no rows, returns empty `Vec`.

</td></tr>

<tr><td>&nbsp;<b><code>Option&lt;_&gt;</code></b></td><td><br>

```rust,ignore
let result = select!(Option<String> "name FROM person")?;
```

Returns `Ok(None)` if no rows, `Error(_)` on error.

</td></tr>

<tr><td><b>&nbsp;Your struct</b></td><td><br>

```rust,ignore
let result = select!(Person "WHERE name = ?", name)?;
```

Column list and table name are optional if type is a `#[derive(Turbosql)]` struct.

```rust,ignore
let result = select!(Vec<NameAndAdult> "name, age >= 18 AS adult FROM person")?;
```

You can use other struct types as well; column names must match the struct and you must specify the source table in the SQL.<br>Implement `Default` to avoid specifying unused column names.<br>(And, of course, you can put it all in a `Vec` or `Option` as well.)

```rust,ignore
let result = select!(Vec<Person>)?;
```

Sometimes everything is optional; this example will retrieve all `Person` rows.

</td></tr>

</table>
<br>

## "turbosql" or "Turbosql"?

Your choice, but you _definitely_ do not want to capitalize any of the _other_ letters in the name! ;)

### License: MIT OR Apache-2.0 OR CC0-1.0 (public domain)
