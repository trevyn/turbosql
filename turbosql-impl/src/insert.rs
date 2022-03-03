use super::Table;
use quote::quote_spanned;

/// INSERT INTO tablename (name1, name2...) VALUES (?1, ?2...)
pub(super) fn insert(table: &Table) -> proc_macro2::TokenStream {
 let sql = makesql_insert(table);

 super::validate_sql_or_abort(&sql);

 let columns = table
  .columns
  .iter()
  .map(|c| {
   let ident = &c.ident;
   quote_spanned!(c.span => &self.#ident as &dyn ::turbosql::ToSql)
  })
  .collect::<Vec<_>>();

 quote_spanned! { table.span =>
  fn insert(&self) -> ::turbosql::Result<i64> {
   assert!(self.rowid.is_none());
   ::turbosql::__TURBOSQL_DB.with(|db| {
    let db = db.borrow_mut();
    let mut stmt = db.prepare_cached(#sql)?;
    stmt.insert(&[#( #columns ),*] as &[&dyn ::turbosql::ToSql])
   })
  }

  fn insert_batch<T: AsRef<#table>>(rows: &[T]) {
   for row in rows {
    row.as_ref().insert().unwrap();
   }
  }
 }
}

fn makesql_insert(table: &Table) -> String {
 let mut sql = format!("INSERT INTO {} (", table.name);
 sql += table.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ").as_str();
 sql += ") VALUES (";
 sql += table.columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ").as_str();
 sql += ")";

 sql
}
