use super::Table;
use quote::quote;

/// SELECT name1, name2... FROM tablename
pub(super) fn select(table: &Table) -> proc_macro2::TokenStream {
 let sql = makesql_select(&table);

 super::validate_sql_or_abort(&sql);

 let quotes = table
  .columns
  .iter()
  .enumerate()
  .map(|(i, c)| {
   let ident = &c.ident;
   quote!(#ident: row.get(#i).unwrap())
  })
  .collect::<Vec<_>>();

 // The ..Default::default() is required to use structs with turbosql(skip) members
 // TODO: this requires #[derive(Turbosql)] structs to also impl/derive Default.

 quote! {
  pub fn select_all() -> Vec<#table> {
   let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
   let mut stmt = db.prepare_cached(#sql).unwrap();

   let iter = stmt.query_map(::turbosql::params![], |row| {
    #[allow(clippy::needless_update)]
    Ok(#table { #(#quotes),* , ..Default::default() })
   }).unwrap();

   let iter = iter.map(|row| row.unwrap());

   iter.collect::<Vec<#table>>()
  }

  pub fn select_where<P>(where_clause: &str, params: P) -> Vec<#table>
  where
  P: IntoIterator,
  P::Item: ::turbosql::ToSql,
  {
   let sql = format!("{} WHERE {}", #sql, where_clause);

   let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
   let mut stmt = db.prepare_cached(&sql).unwrap();

   let iter = stmt.query_map(params, |row| {
    #[allow(clippy::needless_update)]
    Ok(#table { #(#quotes),* , ..Default::default() })
   }).unwrap();

   let iter = iter.map(|row| row.unwrap());

   iter.collect::<Vec<#table>>()
  }

  pub fn __select_sql<P>(sql: &str, params: P) -> Vec<#table>
  where
  P: IntoIterator,
  P::Item: ::turbosql::ToSql,
  {
   let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
   let mut stmt = db.prepare_cached(&sql).unwrap();

   let iter = stmt.query_map(params, |row| {
    #[allow(clippy::needless_update)]
    Ok(#table { #(#quotes),* , ..Default::default() })
   }).unwrap();

   let iter = iter.map(|row| row.unwrap());

   iter.collect::<Vec<#table>>()
  }

  pub fn select_one_where<P>(where_clause: &str, params: P) -> Result<#table, ::turbosql::Error>
  where
  P: IntoIterator,
  P::Item: ::turbosql::ToSql,
  {
   let sql = format!("{} WHERE {} LIMIT 1", #sql, where_clause);
   // trace!("{}", sql);

   let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
   let mut stmt = db.prepare_cached(&sql).unwrap();

   stmt.query_row(params, |row| {
    #[allow(clippy::needless_update)]
    Ok(#table { #(#quotes),* , ..Default::default() })
   })
  }

 }
}

fn makesql_select(table: &Table) -> String {
 let mut sql = "SELECT ".to_string();

 sql += table.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ").as_str();

 sql += format!(" FROM {}", table.name).as_str();

 sql
}
