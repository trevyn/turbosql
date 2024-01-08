// cargo test --features test --manifest-path turbosql/Cargo.toml -- --nocapture --test-threads=1

#![allow(clippy::bool_assert_comparison, clippy::redundant_clone)]

#[cfg(not(feature = "test"))]
compile_error!("turbosql must be tested with '--features test -- --test-threads=1'");
#[cfg(not(test))]
compile_error!("integration_tests.rs must be run in test mode");

use turbosql::{execute, select, update, Blob, Turbosql};

#[derive(Turbosql, Default, Debug, PartialEq, Clone)]
struct PersonIntegrationTest {
	rowid: Option<i64>,
	field_string: Option<String>,
	field_i64: Option<i64>,
	field_bool: Option<bool>,
	field_f64: Option<f64>,
	field_f32: Option<f32>,
	field_u8: Option<u8>,
	field_i8: Option<i8>,
	field_u16: Option<u16>,
	field_i16: Option<i16>,
	field_u32: Option<u32>,
	field_i32: Option<i32>,
	field_blob: Option<Blob>,
	field_vec_u8: Option<Vec<u8>>,
	field_array_u8: Option<[u8; 99]>,
	field_serialize: Option<Vec<i64>>,
	#[turbosql(sql_default = "foo foo")]
	field_string_not_null: String,
	#[turbosql(sql_default = 6)]
	field_i64_not_null: i64,
	#[turbosql(sql_default = false)]
	field_bool_not_null: bool,
	#[turbosql(sql_default = 1.1)]
	field_f64_not_null: f64,
	#[turbosql(sql_default = "2.3")]
	field_f32_not_null: f32,
	#[turbosql(sql_default = -1.2)]
	field_u8_not_null: u8,
	#[turbosql(sql_default = 0)]
	field_i8_not_null: i8,
	#[turbosql(sql_default = r#"hello"#)]
	field_u16_not_null: u16,
	#[turbosql(sql_default = -1)]
	field_i16_not_null: i16,
	#[turbosql(sql_default = -1)]
	field_u32_not_null: u32,
	#[turbosql(sql_default = -1)]
	field_i32_not_null: i32,
	#[turbosql(sql_default = b"")]
	field_blob_not_null: Blob,
	#[turbosql(sql_default = b"0")]
	field_vec_u8_not_null: Vec<u8>,
	#[turbosql(sql_default = b"\x00\x01\xff")]
	field_array_u8_not_null: [u8; 5],
	#[turbosql(sql_default = "")]
	field_serialize_not_null: Vec<i64>,
}

#[test]
fn integration_test() {
	let mut row = PersonIntegrationTest {
		rowid: None,
		field_string: Some("Bob".into()),
		field_u8: Some(42),
		field_i64: Some(85262398562),
		field_f64: Some(std::f64::consts::PI),
		field_f32: Some(std::f32::consts::E),
		field_blob: Some(vec![1, 2, 3]),
		field_array_u8: Some([1u8; 99]),
		field_serialize: Some(vec![42, 43]),
		..Default::default()
	};

	assert_eq!(row.insert().unwrap(), 1);
	assert_eq!(row.insert().unwrap(), 2);
	row.rowid = Some(1);
	row.field_u8 = Some(84);
	assert_eq!(row.update().unwrap(), 1);

	assert_eq!(select!(i64 "1").unwrap(), 1);
	// assert_eq!(select!(Vec<i64> "1").unwrap(), vec![1]);
	// assert_eq!(select!(Option<i64> "1").unwrap(), Some(1));
	assert_eq!(select!(Vec<i64> "rowid FROM personintegrationtest").unwrap(), vec![1, 2]);
	assert_eq!(
		select!(Vec<String> "field_string FROM personintegrationtest").unwrap(),
		vec!["Bob", "Bob"]
	);
	let mut r = PersonIntegrationTest::default();
	let id = r.insert().unwrap();
	let id2 = r.insert().unwrap();
	r.rowid = Some(id);
	r.delete().unwrap();
	assert!(select!(PersonIntegrationTest "WHERE rowid = ?", id).is_err());
	assert!(select!(PersonIntegrationTest "WHERE rowid = ?", id2).is_ok());
	r.rowid = Some(id2);
	r.delete().unwrap();
	assert!(select!(PersonIntegrationTest "WHERE rowid = ?", id2).is_err());
	execute!("DELETE FROM personintegrationtest WHERE rowid = 2").unwrap();
	assert_eq!(select!(i64 "SELECT 1").unwrap(), 1);
	assert_eq!(select!(bool "SELECT 1 > ? AS val", 0).unwrap(), true);
	assert_eq!(select!(bool "SELECT 1 > ? AS val", 2).unwrap(), false);
	assert_eq!(select!(bool "SELECT 1 > " 0 " AS val").unwrap(), true);
	assert_eq!(select!(bool "SELECT 1 > " 2 " AS val").unwrap(), false);
	assert_eq!(select!(bool "SELECT 1 > " 0).unwrap(), true);
	assert_eq!(select!(bool "SELECT 1 > " 2).unwrap(), false);

	assert_eq!(
		format!("{:?}", execute!("")),
		"Err(Rusqlite(SqliteFailure(Error { code: ApiMisuse, extended_code: 21 }, Some(\"not an error\"))))"
	);

	// assert_eq!(select!(Vec<i64> "SELECT 1").unwrap(), Some(1));
	// assert_eq!(select!(Option<i64> "SELECT 1").unwrap(), Some(1));

	assert_eq!(select!(PersonIntegrationTest).unwrap(), row);
	assert_eq!(
		select!(PersonIntegrationTest "rowid, field_string, field_i64, field_bool, field_f64, field_f32, field_u8, field_i8, field_u16, field_i16, field_u32, field_i32, field_blob, field_vec_u8, field_array_u8, field_serialize AS field_serialize__serialized, field_string_not_null, field_i64_not_null, field_bool_not_null, field_f64_not_null, field_f32_not_null, field_u8_not_null, field_i8_not_null, field_u16_not_null, field_i16_not_null, field_u32_not_null, field_i32_not_null, field_blob_not_null, field_vec_u8_not_null, field_array_u8_not_null, field_serialize_not_null AS field_serialize_not_null__serialized FROM personintegrationtest").unwrap(),
		row
	);

	// select! into struct without Turbosql derive

	#[derive(Debug, Eq, PartialEq, Clone, Default)]
	struct NameAndAgeResult {
		name: Option<String>,
		age: Option<i64>,
	}

	assert_eq!(
		select!(NameAndAgeResult r#""Martin Luther" AS name, field_u8 AS age FROM personintegrationtest"#)
			.unwrap(),
		NameAndAgeResult {
			name: Some("Martin Luther".into()),
			age: Some(row.field_u8.unwrap().into())
		}
	);

	assert_eq!(
		select!(Option<NameAndAgeResult> r#""Martin Luther" AS name, field_u8 AS age FROM personintegrationtest"#)
			.unwrap(),
		Some(NameAndAgeResult {
			name: Some("Martin Luther".into()),
			age: Some(row.field_u8.unwrap().into())
		})
	);
	assert_eq!(
		select!(Vec<NameAndAgeResult> r#""Martin Luther" AS name, field_u8 AS age FROM personintegrationtest"#)
			.unwrap(),
		vec![
			NameAndAgeResult {
				name: Some("Martin Luther".into()),
				age: Some(row.field_u8.unwrap().into())
			}
		]
	);

	assert_eq!(select!(Vec<PersonIntegrationTest>).unwrap(), vec![row.clone()]);
	assert_eq!(select!(Option<PersonIntegrationTest>).unwrap(), Some(row.clone()));

	let field_u8 = row.field_u8;
	// assert_eq!(select!(PersonIntegrationTest r#"WHERE field_u8 = $field_u8"#).unwrap(), row);
	assert_eq!(select!(PersonIntegrationTest "WHERE field_u8 = " field_u8).unwrap(), row);
	assert_eq!(
		select!(PersonIntegrationTest "WHERE field_u8 = " field_u8 " AND 1 = " 1).unwrap(),
		row
	);
	assert!(select!(PersonIntegrationTest "WHERE field_u8 = " field_u8 " AND 1 = " 0).is_err());

	let field_string = row.field_string.clone();
	// assert_eq!(select!(PersonIntegrationTest r#"WHERE field_string = $field_string"#).unwrap(), row);
	// assert_eq!(select!(PersonIntegrationTest r#"WHERE field_string = $field_string"#).unwrap(), row);
	assert_eq!(select!(PersonIntegrationTest "WHERE field_string = " field_string " ").unwrap(), row);
	assert_eq!(select!(PersonIntegrationTest "WHERE field_string = " field_string).unwrap(), row);

	// assert_eq!(
	//  select!(PersonIntegrationTest "WHERE field_u8 = $field_u8 AND 1 = ?", 1, 3, 4).unwrap(), row
	// );

	// assert_eq!(select!(PersonIntegrationTest "WHERE 1 = ? AND field_u8 = $field_u8", 1).unwrap(), row);

	assert_eq!(
		select!(PersonIntegrationTest "WHERE field_string = ?", row.field_string.as_ref().unwrap())
			.unwrap(),
		row
	);

	assert_eq!(
		select!(String "field_string FROM personintegrationtest").unwrap(),
		row.field_string.as_deref().unwrap()
	);

	assert_eq!(select!(PersonIntegrationTest "WHERE field_u8 = ?", row.field_u8).unwrap(), row);
	assert_eq!(
		select!(Vec<PersonIntegrationTest> "WHERE field_u8 = ?", row.field_u8).unwrap(),
		vec![row.clone()]
	);
	assert_eq!(
		select!(Option<PersonIntegrationTest> "WHERE field_u8 = ?", row.field_u8).unwrap(),
		Some(row.clone())
	);

	// No rows returned

	assert!(select!(PersonIntegrationTest "WHERE field_u8 = 999").is_err());
	assert_eq!(select!(Vec<PersonIntegrationTest> "WHERE field_u8 = 999").unwrap(), vec![]);
	assert_eq!(select!(Option<PersonIntegrationTest> "WHERE field_u8 = 999").unwrap(), None);

	assert_eq!(select!(f32 "field_f32 FROM personintegrationtest").unwrap(), row.field_f32.unwrap());
	assert_eq!(select!(f64 "field_f64 FROM personintegrationtest").unwrap(), row.field_f64.unwrap());

	assert_eq!(
		select!(i8 "field_u8 FROM personintegrationtest").unwrap(),
		row.field_u8.unwrap() as i8
	);
	assert_eq!(select!(u8 "field_u8 FROM personintegrationtest").unwrap(), row.field_u8.unwrap());
	assert_eq!(
		select!(i16 "field_u8 FROM personintegrationtest").unwrap(),
		row.field_u8.unwrap() as i16
	);
	assert_eq!(
		select!(u16 "field_u8 FROM personintegrationtest").unwrap(),
		row.field_u8.unwrap() as u16
	);
	assert_eq!(
		select!(i32 "field_u8 FROM personintegrationtest").unwrap(),
		row.field_u8.unwrap() as i32
	);
	assert_eq!(
		select!(u32 "field_u8 FROM personintegrationtest").unwrap(),
		row.field_u8.unwrap() as u32
	);
	assert_eq!(
		select!(i64 "field_u8 FROM personintegrationtest").unwrap(),
		row.field_u8.unwrap() as i64
	);
	assert_eq!(
		&select!(Blob "field_blob from personintegrationtest").unwrap(),
		row.field_blob.as_ref().unwrap()
	);
	// assert_eq!(
	// 	&select!([u8; 99] "field_array_u8 from personintegrationtest").unwrap(),
	// 	row.field_array_u8.as_ref().unwrap()
	// );

	assert_eq!(
		select!(bool "field_string = ? FROM personintegrationtest", "Arthur Schopenhauer").unwrap(),
		false
	);
	let new_row = row.clone();
	assert_eq!(
		select!(bool "field_string = ? FROM personintegrationtest", new_row.field_string.unwrap())
			.unwrap(),
		true
	);
	// this incorrectly consumes row:
	// assert_eq!(select!(bool "field_string = ? FROM personintegrationtest", row.field_string.unwrap()).unwrap(), true);

	// select!(PersonIntegrationTest "WHERE field_string = ?", row.field_string.unwrap());

	assert_eq!(
		select!(bool "field_string = ? FROM personintegrationtest", row.clone().field_string.unwrap())
			.unwrap(),
		true
	);

	assert_eq!(
		&select!(String "field_string FROM personintegrationtest").unwrap(),
		row.field_string.as_ref().unwrap()
	);

	assert_eq!(
		select!(Option<i64> "field_u8 FROM personintegrationtest").unwrap(),
		Some(row.field_u8.unwrap() as i64)
	);
	assert!(select!(i64 "field_u8 FROM personintegrationtest WHERE FALSE").is_err());
	assert_eq!(
		select!(Option<i64> "field_u8 FROM personintegrationtest WHERE ?", false).unwrap(),
		None
	);

	assert_eq!(
		select!(Vec<i64> "field_u8 FROM personintegrationtest").unwrap(),
		vec![row.field_u8.unwrap() as i64]
	);
	assert_eq!(select!(Option<u8> "field_u8 FROM personintegrationtest").unwrap(), row.field_u8);
	assert_eq!(
		&select!(String "field_string FROM personintegrationtest").unwrap(),
		row.field_string.as_ref().unwrap()
	);

	// future: tuples:
	// let result = select!((String, i64) "name, age FROM person")?;
	// let result = select!(Vec<(String, i64)> "name, age FROM person")?;
	// let result = select!(Option<(String, i64)> "name, age FROM person")?;

	// struct members
	// assert_eq!(select!(Vec<PersonIntegrationTest.field_u8>).unwrap(), vec![row.field_u8]);
	// assert_eq!(select!(Option<PersonIntegrationTest.field_u8>).unwrap(), Some(row.field_u8));
	// assert_eq!(select!(PersonIntegrationTest.field_u8).unwrap(), row.field_u8);
	// assert_eq!(
	//  select!(Vec<PersonIntegrationTest.field_vec_u8>).unwrap(),
	//  vec![row.field_vec_u8.clone()]
	// );
	// assert_eq!(
	//  select!(Option<PersonIntegrationTest.field_vec_u8>).unwrap(),
	//  Some(row.field_vec_u8.clone())
	// );
	// assert_eq!(select!(PersonIntegrationTest.field_vec_u8).unwrap(), row.field_vec_u8.clone());
	// assert_eq!(select!(Vec<PersonIntegrationTest.field_array_u8>).unwrap(), vec![row.field_array_u8]);
	// assert_eq!(
	//  select!(Option<PersonIntegrationTest.field_array_u8>).unwrap(),
	//  Some(row.field_array_u8)
	// );
	// assert_eq!(select!(PersonIntegrationTest.field_array_u8).unwrap(), row.field_array_u8);

	// assert_eq!(select!(Option<PersonIntegrationTest.field_u8> "WHERE 0").unwrap(), None);

	// let result = select!((Person.name, Person.age))?;
	// let result = select!({Person.name, Person.age})?;
	// let result = select!(Vec<(Person.name, Person.age)>)?;
	// let result = select!(Vec<(Person.name, Person.age)> "WHERE ...")?;

	// let result = select!({ name, "age >= 18 AS" adult: bool } "FROM" Person)?;
	// let result = select!({ Person.name, "age >= 18 AS" adult: bool })?;
	// let result = select!({ name: String, "age >= 18 AS" adult: bool } "FROM person")?;
	// let_select!(name: String, "age >= " adult_age " AS " adult: bool "FROM person")?;
	// let result = select!(( "name AS" String, "age >= 18 AS" bool ) "FROM person")?;

	assert!(
		execute!("INSERT INTO personintegrationtest(field_u8, field_i8) VALUES (" 1, 2 ")").is_err() // NOT NULL constraint failed
	);

	update!("personintegrationtest SET field_u8 = " 0).unwrap();

	// DELETE

	assert!(execute!("DELETE FROM personintegrationtest").is_ok());
	assert!(select!(PersonIntegrationTest).is_err());
}
