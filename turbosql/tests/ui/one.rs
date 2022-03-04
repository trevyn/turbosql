use turbosql::{select, Turbosql};

#[derive(Turbosql, Default)]
struct Person {
 rowid: Option<i64>,
 name: Option<String>,
 age: Option<u8>,
}

fn main() {
 select!(Person "WHERE age = " 24 " AND name = ?", "Bob").unwrap();
}
