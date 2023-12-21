use quote::quote_spanned;

use crate::Table;

pub(super) fn delete(table: &Table) -> proc_macro2::TokenStream {
	let sql = makesql_delete(table);
	super::validate_sql_or_abort(&sql);

	quote_spanned! { table.span =>
		fn delete(&self) -> Result<usize, ::turbosql::Error> {
			assert!(self.rowid.is_some());
			::turbosql::__TURBOSQL_DB.with(|db| {
				let db = db.borrow_mut();
				let mut stmt = db.prepare_cached(#sql)?;
				Ok(stmt.execute([self.rowid])?)
			})
		}
	}
}

fn makesql_delete(table: &Table) -> String {
	format!("DELETE FROM {} WHERE rowid = ?", table.name)
}
