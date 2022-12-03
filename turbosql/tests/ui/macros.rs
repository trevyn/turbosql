// TRYBUILD=overwrite cargo test -p turbosql --test ui

use turbosql::{execute, select, Turbosql};

#[derive(Turbosql, Default)]
struct NoOption {
	rowid: Option<i64>,
	e: u8,
}

#[derive(Turbosql, Default)]
struct NoRowId {
	age: Option<u8>,
}

#[derive(Turbosql, Default)]
struct U64 {
	rowid: Option<i64>,
	e: Option<u64>,
}

#[derive(Turbosql, Default)]
struct Person {
	rowid: Option<i64>,
	name: Option<String>,
	age: Option<u8>,
}

fn main() {
	select!(Person "WHERE age = " 24 " AND name = ?", "Bob").unwrap();
	select!(Person "WHERE age = " 24 " AND name = $name").unwrap();
	select!(Person "WHERE age = ?", 1, 2).unwrap();
	select!(Person "WHERE age = ").unwrap();
	select!("UPDATE person SET age = 1").unwrap();
	execute!("SELECT 1").unwrap();
	select!(Person "WHERE nonexistentcolumn = 1").unwrap();
	select!(Nonexistenttable).unwrap();
	select!(Vec).unwrap();
	select!(Vec<"what">).unwrap();
}
