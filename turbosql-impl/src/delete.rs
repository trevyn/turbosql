use quote::quote_spanned;

use crate::Table;

pub(super) fn delete(table: &Table) -> proc_macro2::TokenStream {
	let sql = makesql_delete(table);
	super::validate_sql_or_abort(&sql);
	let columns = table.columns.iter().filter(|c| c.name == "rowid").map(|c| {
		let ident = &c.ident;
		if c.sql_type == "TEXT" && c.rust_type != "Option < String >" {
			quote_spanned!(c.span => &::turbosql::serde_json::to_string(&self.#ident)? as &dyn ::turbosql::ToSql)
		} else {
			quote_spanned!(c.span => &self.#ident as &dyn ::turbosql::ToSql)
		}
	})
	.collect::<Vec<_>>();


	quote_spanned! { table.span =>
		fn delete(&self) -> Result<i64, ::turbosql::Error> {
			assert!(self.rowid.is_some());
			::turbosql::__TURBOSQL_DB.with(|db| {
				let db = db.borrow_mut();
				let mut stmt = db.prepare_cached(#sql)?;
				Ok(stmt.execute(&[#( #columns ),*] as &[&dyn ::turbosql::ToSql])?)
			})
		}
	}
}

fn makesql_delete(table: &Table) -> String {
	format!("DELETE FROM {} WHERE rowid = ?", table.name)
}
