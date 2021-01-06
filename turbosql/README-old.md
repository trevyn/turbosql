# Turbosql: Easy Data Persistence Layer, backed by SQLite

WORK IN PROGRESS, use at your own risk. :)

Macros for easily persisting Rust `struct`s to an on-disk SQLite database and later retrieving them, optionally based on your own predicates.

## Design Goals

- Low cognitive complexity and low-boilerplate API
- High performance
- Reliable storage
- Surface the power of SQL — make simple things easy, and complex things possible
- In the spirit of Rust, move as many errors as possible to compile time

## Fun Features

- Automatic schema creation and migration, just declare and freely modify your `struct`s.
- Validates all SQL (including user-supplied predicates) at compile time, via a temporary in-memory SQLite database.
- `Blob` type provides efficient incremental BLOB I/O via `Read`, `Write`, and `Seek` traits.
- Specific attention is paid to batch insert performance; small structs can see on the order of ~XXX rows/sec without additional tuning.

## Requirements

Rust 1.45, due to native use of function-like procedural macros in expression position.

# Basic Example

This is a complete, working example — there is no additional boilerplate or setup required.

By default, a SQLite database with the name `[current executable].sqlite` will be created on disk in the current working directory. This can be overridden by calling `turbosql::set_db_path()` before using any Turbosql functionality.

```toml
[dependencies]
turbosql = "0.0.1"
```

```rust
use turbosql::{Turbosql, Blob, select, upsert_batch};

#[derive(Turbosql)]
struct Person {
 rowid: Option<i64>,  // rowid member required & enforced at compile time
 name: String,
 age: Option<i64>,
 image_jpg: Option<Blob>
}

fn main() -> Result<(), turbosql::Error> {
 // INSERT single row -- call insert() with rowid: None
 // TODO: is this optional in the declaration, defaulting to None?
 let person = Person {
  rowid: None,
  name: "Joe",
  age: None,
  image_jpg: None
 };
 let rowid = person.insert()?;

 // SELECT all rows
 let people: Vec<Person> = select!(Vec<Person>)?;

 // SELECT multiple rows with a predicate
 let people: Vec<Person> = select!(Vec<Person> "WHERE age > ?", 21)?;

 // SELECT a single row with a predicate
 let person: Person = select!(Person "WHERE name = ?", "Joe")?;

 // UPDATE single row -- call update() with rowid: Some(i64)
 let mut person = select!(Person "WHERE name = ?", "Joe")?;
 person.age = 18;
 person.update()?;

 // UPSERT batch

 let people: Vec<Person> = vec![person, person];
 upsert_batch!(Person, &people)?;
}
```

## Under the hood

Turbosql generates a SQLite table definition and prepared queries for each struct:

```rust
#[derive(Turbosql)]
struct Person {
 rowid: Option<i64>,  // rowid member required & enforced
 name: String,
 age: Option<i64>,
 image_jpg: Option<Blob>
}
```

&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;↓&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;generates and validates the SQL

```sqlite3
CREATE TABLE person (
 rowid INTEGER PRIMARY KEY,
 name TEXT,
 age INTEGER,
 image_jpg BLOB,
)

INSERT INTO person (rowid, name, age, image_jpg) VALUES (?, ?, ?, ?)

SELECT rowid, name, age, image_jpg FROM person
```

(For various reasons, the underlying schema does not use `NOT NULL` for non-optional members, but `SELECT`s will return an error if a SQL `NULL` value is returned for a non-optional member.)

Queries with SQL predicates are also generated and validated at compile time. Note that SQL types vs Rust types are not currently checked.

```rust
let people = select!(Vec<Person> "WHERE age > ?", 21);

let person = select!(Person "WHERE name = ?", "Joe");
```

&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;↓

```sqlite3
SELECT rowid, name, age, image_jpg FROM person WHERE age > ?

SELECT rowid, name, age, image_jpg FROM person WHERE name = ? LIMIT 1
```

# Details

## Automatic Schema Migrations

At compile time, the derive macro creates a `migrations.toml` file in your project root that describes the database schema. Each time you change a `struct` declaration and recompile, migration SQL statements are generated that update the database schema. These statements are recorded in `migrations.toml`, and are automatically included in your binary.

```rust
#[derive(Turbosql)]
struct Person {
 rowid: Option<i64>,
 name: String
}
```

&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;↓&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;generates `migrations.toml`

```toml
[[table]]
migrations = [
  'CREATE TABLE person(rowid INTEGER PRIMARY KEY)',
  'ALTER TABLE person ADD COLUMN name TEXT',
]
name = 'person'
schema = '''
CREATE TABLE person (
 rowid INTEGER PRIMARY KEY,
 name TEXT
)'''
```

If the binary's schema is newer than the schema of the database file, the new migrations will be automatically applied in sequence, and the resulting schema will be verified to be precisely equal to the expected schema. By default, this completes in O(1) time. (See SQLite docs.) Optionally, deleted columns can be set to `NULL`. (SQLite requires a complete table rebuild to drop columns, so instead we just rename to `xxx_deleted`.)

Currently, this migration process is one-way; if your schema changes, any future version of your binary will migrate any older database file to the current schema, but this will prevent older versions of the binary from reading the now-updated database file.

Unused or reverted migrations that are created during development can be manually removed from `migrations.toml` before being released, but any database files that have already applied these deleted migrations will error and must be rebuilt. Proceed with care. When in doubt, refrain from manually editing `migrations.toml`, and everything should work fine.

Caution: Renaming a struct member is interpreted as a column deletion and a column addition, so data stored in the old column will be deleted on migration.

## Additional Examples

- ### Blob I/O

# Commentary

## Why return results in a `Vec` instead of an iterator?

Keeping the SQLite query open for longer than necessary has concurrency implications for any additional queries to the database that may arrive during this time. To keep things simple, we fully encapsulate queries in a single blocking call, forcing serialization. If this leads to an actual performance issue for your use case, please open a GitHub issue with details.

## `-wal` and `-shm` files

SQLite is an extremely reliable database engine, provided you understand how it interfaces with the filesystem. The main `.sqlite` file contains the bulk of the database. During database writes, SQLite also creates `.sqlite-wal` and `.sqlite-shm` files. If the process is terminated without flushing writes, you may end up with these three files when you expected one. This is fine; on next launch, SQLite knows how to resolve interrupted writes. However, if the `-wal` and/or `-shm` files are present, they **must be considered essential parts of the database**. Deleting them by hand may result in a corrupted database. See https://sqlite.org/tempfiles.html .

# Current Limitations

- When using a `struct` type in the `select!` macro, namespaces are not handled; the `struct` must be imported such that it can be referenced without any additional scoping:

  ```rust
  select!(Person "WHERE name = ?", "Joe")  // Good
  select!(super::Person "WHERE name = ?", "Joe")  // Does not work yet
  ```

# Future directions

## Fancy return types

```rust
let count: u64 = select!(u64 "COUNT(*) FROM person").unwrap();

let name: String = select!(Person.name "WHERE rowid = ?", 1).unwrap();

let names: Vec<String> = select!(Vec<Person.name> "WHERE age = ?", 21).unwrap();

let names_ages: Vec<(String, i64)> = select!(Vec<(Person.name, Person.age)>).unwrap();

let all_names: Vec<String> = select!(Vec<Person.name>).unwrap();
```

&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;↓

```sqlite3
SELECT COUNT(*) from person

SELECT name FROM person WHERE rowid = ? LIMIT 1

SELECT name FROM person WHERE age = ?

SELECT name, age FROM person

SELECT name FROM person
```

## Type assistance

We'd like to be able to do something like below, with `age_t` and `name_t` representing the types of these members, and these types being checked against the macro parameters at compile time.

This should be possible, as the `select!` macro has access to member type data, passed from the derive macro, which can be explicitly included in the `params![]` expansion.

```rust
let people = select!(Vec<Person> "WHERE age > ?{age_t}", 18);

let person = select!(Person "WHERE name = ?{name_t}", "Joe");
```

## Exact count selection

```rust
let person = select!(exactly 1 Person "WHERE name = ?", "Joe");
let person = select!(exactly n Person "WHERE name = ?", "Joe");
```

Use `LIMIT n+1` in the underlying SQL query and return error if count(rows) != n

## Serialize/deserialize arbitrary serializable types to BLOB

With serde

## Allow using multiple database files
