use super::Table;
use quote::quote_spanned;

/// INSERT INTO tablename (name1, name2...) VALUES (?1, ?2...)
pub(super) fn insert(table: &Table) -> proc_macro2::TokenStream {
 let sql = makesql_insert(&table);

 super::validate_sql_or_abort(&sql);

 // let idents = table.columns.iter().map(|c| &c.ident).collect::<Vec<_>>();
 let columns = table
  .columns
  .iter()
  .map(|c| {
   let ident = &c.ident;
   quote_spanned!(c.span=> &self.#ident as &dyn ::turbosql::ToSql)
  })
  .collect::<Vec<_>>();

 quote_spanned! { table.span =>
  #[allow(dead_code)]
  pub fn insert(&self) -> ::turbosql::Result<usize> {
   // #table::__turbosql_ensure_table_created();
   assert!(self.rowid.is_none());
   let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();  // todo: use tokio's lock?
   let mut stmt = db.prepare_cached(#sql)?;
   stmt.execute(&[#(#columns),*] as &[&dyn ::turbosql::ToSql])
  }

  #[allow(dead_code)]
  pub fn insert_batch(rows: &[#table]) {
   for row in rows {
    row.insert().unwrap();
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
