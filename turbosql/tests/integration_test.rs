use turbosql::{execute, select, Blob, Turbosql};

#[derive(Turbosql, Default, Debug, PartialEq, Clone)]
struct PersonIntegrationTest {
 rowid: Option<i64>,
 name: Option<String>,
 age: Option<i64>,
 image_jpg: Option<Blob>,
}

/* @test integration test
cd turbosql
cargo test --features test -- --nocapture
*/
#[test]
fn it_works() {
 let mut row = PersonIntegrationTest {
  rowid: None,
  name: Some("Bob".to_string()),
  age: Some(42),
  image_jpg: None,
 };

 row.insert().unwrap();

 row.rowid = Some(1);

 assert!(select!(i64 "1").unwrap() == 1);
 assert!(select!(i64 "SELECT 1").unwrap() == 1);
 assert!(
  execute!("")
   == Err(rusqlite::Error::SqliteFailure(
    rusqlite::ffi::Error { code: rusqlite::ErrorCode::APIMisuse, extended_code: 21 },
    Some("not an error".to_string()),
   ))
 );

 // assert!(select!(Vec<i64> "SELECT 1").unwrap() == Some(1));
 // assert!(select!(Option<i64> "SELECT 1").unwrap() == Some(1));

 assert!(select!(PersonIntegrationTest).unwrap() == row);
 assert!(select!(Vec<PersonIntegrationTest>).unwrap() == vec![row.clone()]);
 assert!(select!(Option<PersonIntegrationTest>).unwrap() == Some(row.clone()));

 assert!(select!(PersonIntegrationTest "WHERE age = ?", row.age).unwrap() == row);
 assert!(
  select!(Vec<PersonIntegrationTest> "WHERE age = ?", row.age).unwrap() == vec![row.clone()]
 );
 assert!(
  select!(Option<PersonIntegrationTest> "WHERE age = ?", row.age).unwrap() == Some(row.clone())
 );

 assert!(select!(PersonIntegrationTest "WHERE age = 41").is_err());
 assert!(select!(Vec<PersonIntegrationTest> "WHERE age = 41").unwrap() == vec![]);
 assert!(select!(Option<PersonIntegrationTest> "WHERE age = 41").unwrap() == None);

 assert!(select!(i64 "SELECT age FROM personintegrationtest").unwrap() == row.age.unwrap());
 assert!(select!(i64 "age FROM personintegrationtest").unwrap() == row.age.unwrap());
 assert!(select!(i64 "age FROM personintegrationtest WHERE FALSE").is_err());
 // assert!(select!(Vec<i64> "age FROM personintegrationtest").unwrap() == row.age.unwrap());
 // assert!(select!(Option<i64> "age FROM personintegrationtest").unwrap() == row.age);
 // assert!(select!(String "name FROM personintegrationtest").unwrap() == row.name.unwrap());
}

#[test]
#[should_panic]
fn it_panics() {
 panic!("panic");
}
