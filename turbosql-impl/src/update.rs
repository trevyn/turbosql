use super::Table;
use proc_macro_error::abort_call_site;
use quote::quote_spanned;

/// UPDATE tablename SET name1=?, name2=?... WHERE rowid=?
pub(super) fn update(table: &Table) -> proc_macro2::TokenStream {
 if table.columns[0].name != "rowid" {
  abort_call_site!("First field must be `rowid: Option<i64>`");
 }

 let sql = makesql_update(table);

 super::validate_sql_or_abort(&sql);

 let mut columns = table.columns.clone();
 columns.rotate_left(1);
 let columns = columns
  .iter()
  .map(|c| {
   let ident = &c.ident;
   quote_spanned!(c.span => &self.#ident as &dyn ::turbosql::ToSql)
  })
  .collect::<Vec<_>>();

 quote_spanned! { table.span =>
  #[allow(dead_code)]
  pub fn update(&self) -> ::turbosql::Result<usize> {
   assert!(self.rowid.is_some());
   ::turbosql::__TURBOSQL_DB.with(|db| {
    let db = db.borrow_mut();
    let mut stmt = db.prepare_cached(#sql)?;
    stmt.execute(&[#( #columns ),*] as &[&dyn ::turbosql::ToSql])
   })
  }

  #[allow(dead_code)]
  pub fn update_batch(rows: &[#table]) {
   for row in rows {
    row.update().unwrap();
   }
  }
 }
}

fn makesql_update(table: &Table) -> String {
 format!(
  "UPDATE {} SET {} WHERE rowid=?",
  table.name,
  table.columns.iter().collect::<Vec<_>>()[1..]
   .iter()
   .map(|c| format!("{}=?", c.name.as_str()))
   .collect::<Vec<_>>()
   .join(", ")
 )
}
