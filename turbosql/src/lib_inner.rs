use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// these re-exports are used in macro expansions

#[doc(hidden)]
pub use once_cell::sync::Lazy;
#[doc(hidden)]
pub use rusqlite::{
	self, named_params, params, types::FromSql, types::FromSqlResult, types::ToSql,
	types::ToSqlOutput, types::Value, types::ValueRef,
};
#[doc(hidden)]
pub use serde::Serialize;
#[doc(hidden)]
pub use serde_json;
pub use turbosql_impl::{execute, select, update, Turbosql};

/// Wrapper for `Vec<u8>` that may one day impl `Read`, `Write` and `Seek` traits.
pub type Blob = Vec<u8>;

/// `#[derive(Turbosql)]` generates impls for this trait.
pub trait Turbosql {
	/// Inserts this row into the database. `rowid` must be `None`. On success, returns the new `rowid`.
	fn insert(&self) -> Result<i64, Error>;
	fn insert_batch<T: AsRef<Self>>(rows: &[T]) -> Result<(), Error>;
	/// Updates this existing row in the database, based on `rowid`, which must be `Some`. All fields are overwritten in the database. On success, returns the number of rows updated, which should be 1.
	fn update(&self) -> Result<usize, Error>;
	fn update_batch<T: AsRef<Self>>(rows: &[T]) -> Result<(), Error>;
	/// Deletes this existing row in the database, based on `rowid`, which must be `Some`. On success, returns the number of rows deleted, which should be 1.
	fn delete(&self) -> Result<usize, Error>;
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error(transparent)]
	Rusqlite(#[from] rusqlite::Error),
	#[error(transparent)]
	SerdeJson(#[from] serde_json::Error),
	#[error("Turbosql Error: {0}")]
	OtherError(&'static str),
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, Default)]
struct MigrationsToml {
	migrations_append_only: Option<Vec<String>>,
	output_generated_schema_for_your_information_do_not_edit: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct DbPath {
	path: Option<PathBuf>,
	opened: bool,
}

static __DB_PATH: Lazy<Mutex<DbPath>> = Lazy::new(Default::default);

/// Convenience function that returns the current time as milliseconds since UNIX epoch.
pub fn now_ms() -> i64 {
	std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

/// Convenience function that returns the current time as microseconds since UNIX epoch.
pub fn now_us() -> i64 {
	std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as i64
}

/// Returns the path to the database, if it has been set or opened.
pub fn db_path() -> Option<PathBuf> {
	__DB_PATH.lock().unwrap().path.clone()
}

fn run_migrations(conn: &mut Connection, path: &Path) {
	cfg_if::cfg_if! {
		if #[cfg(doc)] {
			// if these are what's run in doctests, could add a test struct here to scaffold one-liner tests
			let toml_decoded: MigrationsToml = MigrationsToml::default();
		} else if #[cfg(feature = "test")] {
			let toml_decoded: MigrationsToml = toml::from_str(include_str!("../../test.migrations.toml")).unwrap();
		} else {
			let toml_decoded: MigrationsToml = toml::from_str(include_str!(concat!(env!("OUT_DIR"), "/migrations.toml"))).expect("Unable to decode embedded migrations.toml");
		}
	};

	let target_migrations = toml_decoded.migrations_append_only.unwrap_or_default();

	// filter out comments
	let target_migrations: Vec<_> =
		target_migrations.into_iter().filter(|m| !m.starts_with("--")).collect();

	conn.execute("BEGIN EXCLUSIVE TRANSACTION", params![]).unwrap();

	let _ = conn.execute("ALTER TABLE turbosql_migrations RENAME TO _turbosql_migrations", params![]);

	let result = conn.query_row(
		"SELECT sql FROM sqlite_master WHERE name = ?",
		params!["_turbosql_migrations"],
		|row| {
			let sql: String = row.get(0).unwrap();
			Ok(sql)
		},
	);

	match result {
		Err(rusqlite::Error::QueryReturnedNoRows) => {
			// no migrations table exists yet, create
			conn
				.execute_batch(if cfg!(feature = "sqlite-compat-no-strict-tables") {
					r#"CREATE TABLE _turbosql_migrations (rowid INTEGER PRIMARY KEY, migration TEXT NOT NULL)"#
				} else {
					r#"CREATE TABLE _turbosql_migrations (rowid INTEGER PRIMARY KEY, migration TEXT NOT NULL) STRICT"#
				})
				.expect("CREATE TABLE _turbosql_migrations");
		}
		Err(err) => {
			panic!("Could not query sqlite_master table: {}", err);
		}
		Ok(_) => (),
	}

	let applied_migrations = conn
		.prepare("SELECT migration FROM _turbosql_migrations ORDER BY rowid")
		.unwrap()
		.query_map(params![], |row| Ok(row.get(0).unwrap()))
		.unwrap()
		.map(|x: Result<String, _>| x.unwrap())
		.filter(|m| !m.starts_with("--"))
		.collect::<Vec<String>>();

	// execute migrations

	let mut a = applied_migrations.iter();
	let mut t = target_migrations.iter();

	loop {
		match (a.next(), t.next()) {
			(Some(a), Some(t)) => {
				if a != t {
					panic!("Mismatch in Turbosql migrations! {:?} != {:?} {:?}", a, t, path)
				}
			}
			(Some(a), None) => {
				panic!(
					"Mismatch in Turbosql migrations! More migrations are applied than target. {:?} {:?}",
					a, path
				)
			}
			(None, Some(t)) => {
				if !t.starts_with("--") {
					conn.execute(t, params![]).unwrap();
				}
				conn.execute("INSERT INTO _turbosql_migrations(migration) VALUES(?)", params![t]).unwrap();
			}
			(None, None) => break,
		}
	}

	// TODO: verify schema against output_generated_schema_for_your_information_do_not_edit

	//    if sql != create_sql {
	//     println!("{}", sql);
	//     println!("{}", create_sql);
	//     panic!("Turbosql sqlite schema does not match! Delete database file to continue.");
	//    }

	conn.execute("COMMIT", params![]).unwrap();
}

#[derive(Debug)]
pub struct CheckpointResult {
	/// Should always be 0. (Checkpoint is run in PASSIVE mode.)
	pub busy: i64,
	/// The number of modified pages that have been written to the write-ahead log file.
	pub log: i64,
	/// The number of pages in the write-ahead log file that have been successfully moved back into the database file at the conclusion of the checkpoint.
	pub checkpointed: i64,
}

/// Checkpoint the DB.
/// If no other threads have open connections, this will clean up the `-wal` and `-shm` files as well.
pub fn checkpoint() -> Result<CheckpointResult, Error> {
	let start = std::time::Instant::now();
	let db_path = __DB_PATH.lock().unwrap();

	let conn = Connection::open_with_flags(
		db_path.path.as_ref().unwrap(),
		OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
	)?;

	let result = conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", params![], |row| {
		Ok(CheckpointResult { busy: row.get(0)?, log: row.get(1)?, checkpointed: row.get(2)? })
	})?;

	log::info!("db checkpointed in {:?} {:#?}", start.elapsed(), result);

	Ok(result)
}

fn open_db() -> Connection {
	let mut db_path = __DB_PATH.lock().unwrap();

	if db_path.path.is_none() {
		#[cfg(not(feature = "test"))]
		let path = {
			let exe_stem = std::env::current_exe().unwrap().file_stem().unwrap().to_owned();
			let exe_stem_lossy = exe_stem.to_string_lossy();

			let path = directories_next::ProjectDirs::from("org", &exe_stem_lossy, &exe_stem_lossy)
				.unwrap()
				.data_dir()
				.to_owned();

			std::fs::create_dir_all(&path).unwrap();

			path.join(exe_stem).with_extension("sqlite")
		};

		#[cfg(feature = "test")]
		let path = Path::new(":memory:").to_owned();

		db_path.path = Some(path);
	}

	log::debug!("opening db at {:?}", db_path.path.as_ref().unwrap());

	// We are handling the mutex by being thread_local, so SQLite can be opened in no-mutex mode; see:
	// https://www.mail-archive.com/sqlite-users@mailinglists.sqlite.org/msg112907.html

	let mut conn = Connection::open_with_flags(
		db_path.path.as_ref().unwrap(),
		OpenFlags::SQLITE_OPEN_READ_WRITE
			| OpenFlags::SQLITE_OPEN_CREATE
			| OpenFlags::SQLITE_OPEN_NO_MUTEX,
	)
	.expect("rusqlite::Connection::open_with_flags");

	conn
		.execute_batch(
			r#"
				PRAGMA busy_timeout=3000;
				PRAGMA auto_vacuum=INCREMENTAL;
				PRAGMA journal_mode=WAL;
				PRAGMA wal_autocheckpoint=8000;
				PRAGMA synchronous=NORMAL;
			"#,
		)
		.expect("Execute PRAGMAs");

	if !db_path.opened {
		run_migrations(&mut conn, db_path.path.as_ref().unwrap());
		db_path.opened = true;
	}

	conn
}

thread_local! {
	#[doc(hidden)]
	pub static __TURBOSQL_DB: RefCell<Connection> = RefCell::new(open_db());
}

/// Set the local path and filename where Turbosql will store the underlying SQLite database.
///
/// Must be called before any usage of Turbosql macros or will return an error.
pub fn set_db_path(path: &Path) -> Result<(), Error> {
	let mut db_path = __DB_PATH.lock().unwrap();

	if db_path.opened {
		return Err(Error::OtherError("Trying to set path when DB is already opened"));
	}

	db_path.path = Some(path.to_owned());

	Ok(())
}
