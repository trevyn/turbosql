use i54_::i54;
use turbosql::{execute, select, Blob, Turbosql};

#[derive(Turbosql, Default, Debug, Eq, PartialEq, Clone)]
struct PersonIntegrationTest {
 rowid: Option<i64>,
 name: Option<String>,
 age: Option<i64>,
 image_jpg: Option<Blob>,
}

#[derive(Turbosql, Default, Debug, Eq, PartialEq, Clone)]
#[allow(non_camel_case_types)]
struct PersonIntegrationTest_i54 {
 rowid: Option<i54>,
 name: Option<String>,
 age: Option<i64>,
 image_jpg: Option<Blob>,
}

// @test integration test
// cargo test --features test --manifest-path turbosql/Cargo.toml -- --nocapture
#[test]
fn integration_test() {
 let mut row =
  PersonIntegrationTest { rowid: None, name: Some("Bob".into()), age: Some(42), image_jpg: None };

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
 assert!(
  select!(PersonIntegrationTest "rowid, name, age, image_jpg FROM personintegrationtest").unwrap()
   == row
 );

 // select! into struct without Turbosql derive

 #[derive(Debug, Eq, PartialEq, Clone)]
 struct NameAndAgeResult {
  name: Option<String>,
  age: Option<i64>,
 }

 assert!(
  select!(NameAndAgeResult r#""Martin Luther" AS name, age FROM personintegrationtest"#).unwrap()
   == NameAndAgeResult { name: Some("Martin Luther".into()), age: row.age }
 );

 assert!(select!(Vec<PersonIntegrationTest>).unwrap() == vec![row.clone()]);
 assert!(select!(Option<PersonIntegrationTest>).unwrap() == Some(row.clone()));

 assert!(select!(PersonIntegrationTest "WHERE age = ?", row.age).unwrap() == row);
 assert!(
  select!(Vec<PersonIntegrationTest> "WHERE age = ?", row.age).unwrap() == vec![row.clone()]
 );
 assert!(
  select!(Option<PersonIntegrationTest> "WHERE age = ?", row.age).unwrap() == Some(row.clone())
 );

 // No rows returned

 assert!(select!(PersonIntegrationTest "WHERE age = 999").is_err());
 assert!(select!(Vec<PersonIntegrationTest> "WHERE age = 999").unwrap() == vec![]);
 assert!(select!(Option<PersonIntegrationTest> "WHERE age = 999").unwrap() == None);

 assert!(select!(i8 "age FROM personintegrationtest").unwrap() == row.age.unwrap() as i8);
 assert!(select!(u8 "age FROM personintegrationtest").unwrap() == row.age.unwrap() as u8);
 assert!(select!(i16 "age FROM personintegrationtest").unwrap() == row.age.unwrap() as i16);
 assert!(select!(u16 "age FROM personintegrationtest").unwrap() == row.age.unwrap() as u16);
 assert!(select!(i32 "age FROM personintegrationtest").unwrap() == row.age.unwrap() as i32);
 assert!(select!(u32 "age FROM personintegrationtest").unwrap() == row.age.unwrap() as u32);
 assert!(select!(i64 "age FROM personintegrationtest").unwrap() == row.age.unwrap());

 assert!(
  select!(bool "name = ? FROM personintegrationtest", "Arthur Schopenhauer").unwrap() == false
 );
 let new_row = row.clone();
 assert!(
  select!(bool "name = ? FROM personintegrationtest", new_row.name.unwrap()).unwrap() == true
 );
 // this incorrectly consumes row:
 // assert!(select!(bool "name = ? FROM personintegrationtest", row.name.unwrap()).unwrap() == true);

 // select!(PersonIntegrationTest "WHERE name = ?", row.name.unwrap());

 assert!(
  select!(bool "name = ? FROM personintegrationtest", row.clone().name.unwrap()).unwrap() == true
 );

 assert!(select!(String "name FROM personintegrationtest").unwrap() == row.name.unwrap());

 // assert!(select!(Option<i64> "age FROM personintegrationtest").unwrap() == Some(row.age.unwrap()));
 assert!(select!(i64 "age FROM personintegrationtest WHERE FALSE").is_err());
 // assert!(select!(Option<i64> "age FROM personintegrationtest WHERE ?", false).unwrap() == None);

 // assert!(select!(Vec<i64> "age FROM personintegrationtest").unwrap() == row.age.unwrap());
 // assert!(select!(Option<i64> "age FROM personintegrationtest").unwrap() == row.age);
 // assert!(select!(String "name FROM personintegrationtest").unwrap() == row.name.unwrap());

 // DELETE

 assert!(execute!("DELETE FROM personintegrationtest").is_ok());
 assert!(select!(PersonIntegrationTest).is_err());
}
