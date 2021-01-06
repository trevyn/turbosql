//! This crate provides Turbosql's procedural macros.
//!
//! Please refer to the `turbosql` crate for how to set this up.

// #![allow(unused_imports)]
const SQLITE_64BIT_ERROR: &str = r##"Sadly, SQLite cannot natively store unsigned 64-bit integers, so TurboSQL does not support u64 members. Use i64, u32, f64, or a string or binary format instead. (see https://sqlite.org/fileformat.html#record_format )"##;

use once_cell::sync::Lazy;
use proc_macro2::Span;
use proc_macro_error::{abort, abort_call_site, proc_macro_error};
use quote::{format_ident, quote, ToTokens};
use rusqlite::{params, Connection, Statement};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
 parse_macro_input, Data, DeriveInput, Expr, Fields, FieldsNamed, Ident, LitStr, Meta, NestedMeta,
 Token, Type,
};

#[cfg(not(feature = "test"))]
const MIGRATIONS_FILENAME: &str = "migrations.toml";
#[cfg(feature = "test")]
const MIGRATIONS_FILENAME: &str = "test.migrations.toml";

mod create;
mod insert;
mod select;

// trait Ok<T> {
//  fn ok(self) -> Result<T, anyhow::Error>;
// }

// impl<T> Ok<T> for Option<T> {
//  fn ok(self) -> Result<T, anyhow::Error> {
//   self.ok_or_else(|| anyhow::anyhow!("NoneError"))
//  }
// }

#[derive(Debug, Clone)]
struct Table {
 ident: Ident,
 span: Span,
 name: String,
 columns: Vec<Column>,
}

#[derive(Debug)]
struct MiniTable {
 name: String,
 columns: Vec<MiniColumn>,
}

impl ToTokens for Table {
 fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
  let ident = &self.ident;
  tokens.extend(quote!(#ident));
 }
}

#[derive(Debug, Clone)]
struct Column {
 ident: Ident,
 span: Span,
 name: String,
 rust_type: String,
 sql_type: &'static str,
}

#[derive(Debug)]
struct MiniColumn {
 name: String,
 rust_type: String,
 sql_type: &'static str,
}

// static TEST_DB: Lazy<Mutex<Connection>> =
//  Lazy::new(|| Mutex::new(Connection::open_in_memory().unwrap()));

static LAST_TABLE_NAME: Lazy<Mutex<String>> = Lazy::new(|| Mutex::new("none".to_string()));

static TABLES: Lazy<Mutex<HashMap<String, MiniTable>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// #[proc_macro]
// pub fn set_db_path(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
//  let input = proc_macro2::TokenStream::from(input);

//  eprintln!("IN SET DB PATH!");
//  eprintln!("{:#?}", input);

//  let mut db_path = DB_PATH.lock().unwrap();

//  let mut iter = input.into_iter();

//  *db_path = match iter.next() {
//   Some(proc_macro2::TokenTree::Literal(literal)) => literal.to_string(),
//   _ => panic!("Expected string literal"),
//  };

//  proc_macro::TokenStream::new()
// }

#[derive(Debug)]
struct SelectTokens {
 tokens: proc_macro2::TokenStream,
}

#[derive(Debug)]
struct ExecuteTokens {
 tokens: proc_macro2::TokenStream,
}

#[derive(Debug)]
struct QueryParams {
 params: Punctuated<Expr, Token![,]>,
}

impl Parse for QueryParams {
 fn parse(input: ParseStream) -> syn::Result<Self> {
  Ok(QueryParams {
   params: if input.peek(Token![,]) {
    input.parse::<Token![,]>().unwrap();
    input.parse_terminated(Expr::parse)?
   } else {
    Punctuated::new()
   },
  })
 }
}

#[derive(Clone, Debug)]
struct ResultType {
 container: Option<Ident>,
 contents: Option<Ident>,
}

// impl Parse for ResultType {
//  fn parse(input: ParseStream) -> syn::Result<Self> {
//   let path = input.parse::<syn::Path>();
//   eprintln!("{:#?}", path);
//   Ok(ResultType {})
//  }
// }

#[derive(Debug)]
struct MembersAndCasters {
 members: Vec<(Ident, Ident, usize)>,
 struct_members: Vec<proc_macro2::TokenStream>,
 row_casters: Vec<proc_macro2::TokenStream>,
}

impl MembersAndCasters {
 fn create(members: Vec<(Ident, Ident, usize)>) -> MembersAndCasters {
  let struct_members: Vec<_> = members.iter().map(|(name, ty, _i)| quote!(#name: #ty)).collect();
  let row_casters =
   members.iter().map(|(name, _ty, i)| quote!(#name: row.get(#i)?)).collect::<Vec<_>>();

  Self { members, struct_members, row_casters }
 }
}

fn extract_explicit_members(columns: &[String]) -> Option<MembersAndCasters> {
 // let members: Vec<_> = columns
 //  .iter()
 //  .enumerate()
 //  .filter_map(|(i, cap)| {
 //   let col_name = cap;
 //   let mut parts: Vec<_> = col_name.split('_').collect();
 //   if parts.len() < 2 {
 //    return None;
 //   }
 //   let ty = parts.pop()?;
 //   let name = parts.join("_");
 //   Some((format_ident!("{}", name), format_ident!("{}", ty), i))
 //  })
 //  .collect();

 println!("extractexplicitmembers: {:#?}", columns);

 // MembersAndCasters::create(members);
 // syn::parse_str::<Ident>

 None
}

fn extract_stmt_members(stmt: &Statement, span: &Span) -> MembersAndCasters {
 let members: Vec<_> = stmt
  .column_names()
  .iter()
  .enumerate()
  .map(|(i, col_name)| {
   let mut parts: Vec<_> = col_name.split('_').collect();

   if parts.len() < 2 {
    abort!(
     span,
     "SQL column name {:#?} must include a type annotation, e.g. {}_String or {}_i64.",
     col_name,
     col_name,
     col_name
    )
   }

   let ty = parts.pop().unwrap();

   match ty {
    "i64" | "String" => (),
    _ => abort!(span, "Invalid type annotation \"_{}\", try e.g. _String or _i64.", ty),
   }

   let name = parts.join("_");

   (format_ident!("{}", name), format_ident!("{}", ty), i)
  })
  .collect();

 // let struct_members: Vec<_> = members.iter().map(|(name, ty, _i)| quote!(#name: #ty)).collect();
 // let row_casters: Vec<_> =
 //  members.iter().map(|(name, _ty, i)| quote!(#name: row.get(#i).unwrap())).collect();

 MembersAndCasters::create(members)
}

enum ParseStatementType {
 Execute,
 Select,
}
use ParseStatementType::{Execute, Select};

#[derive(Debug)]
struct StatementInfo {
 parameter_count: usize,
 column_names: Vec<String>,
}

impl StatementInfo {
 fn membersandcasters(&self) -> syn::parse::Result<MembersAndCasters> {
  Ok(MembersAndCasters::create(
   self
    .column_names
    .iter()
    .enumerate()
    .map(|(i, col_name)| Ok((syn::parse_str::<Ident>(col_name)?, format_ident!("None"), i)))
    .collect::<syn::parse::Result<Vec<_>>>()?,
  ))
 }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct MigrationsToml {
 migrations_append_only: Option<Vec<String>>,
 target_schema_autogenerated: Option<String>,
}

fn migrations_to_tempdb(migrations: &[String]) -> Connection {
 let tempdb = rusqlite::Connection::open_in_memory().unwrap();

 tempdb
  .execute_batch(
   "CREATE TABLE turbosql_migrations (rowid INTEGER PRIMARY KEY, migration TEXT NOT NULL);",
  )
  .unwrap();

 migrations.iter().for_each(|m| match tempdb.execute(m, params![]) {
  Ok(_) => (),
  Err(rusqlite::Error::ExecuteReturnedResults) => (), // pragmas
  Err(e) => abort_call_site!("Running migrations on temp db: {:?}", e),
 });

 tempdb
}

fn migrations_to_schema(migrations: &[String]) -> Result<String, rusqlite::Error> {
 Ok(
  migrations_to_tempdb(migrations)
   .prepare("SELECT sql FROM sqlite_master WHERE type='table' ORDER BY sql")?
   .query_map(params![], |row| Ok(row.get(0)?))?
   .collect::<Result<Vec<String>, _>>()?
   .join("\n"),
 )
}

fn read_migrations_toml() -> MigrationsToml {
 let lockfile = std::fs::File::create(std::env::temp_dir().join("migrations.toml.lock")).unwrap();
 fs2::FileExt::lock_exclusive(&lockfile).unwrap();

 let migrations_toml_path = std::env::current_dir().unwrap().join(MIGRATIONS_FILENAME);
 let migrations_toml_path_lossy = migrations_toml_path.to_string_lossy();

 match migrations_toml_path.exists() {
  true => {
   let toml_str = std::fs::read_to_string(&migrations_toml_path)
    .unwrap_or_else(|e| abort_call_site!("Unable to read {}: {:?}", migrations_toml_path_lossy, e));

   let toml_decoded: MigrationsToml = toml::from_str(&toml_str).unwrap_or_else(|e| {
    abort_call_site!("Unable to decode toml in {}: {:?}", migrations_toml_path_lossy, e)
   });

   toml_decoded
  }
  false => MigrationsToml::default(),
 }
}

fn validate_sql<S: AsRef<str>>(sql: S) -> Result<StatementInfo, rusqlite::Error> {
 let tempdb = migrations_to_tempdb(&read_migrations_toml().migrations_append_only.unwrap());

 let stmt = tempdb.prepare(sql.as_ref());

 eprintln!("{:#?}", stmt);

 let stmt = stmt?;

 Ok(StatementInfo {
  parameter_count: stmt.parameter_count(),
  column_names: stmt.column_names().into_iter().map(str::to_string).collect(),
 })
}

fn validate_sql_or_abort<S: AsRef<str> + std::fmt::Debug>(sql: S) -> StatementInfo {
 validate_sql(sql.as_ref()).unwrap_or_else(|e| {
  abort_call_site!(r#"Error validating SQL statement: "{}". SQL: {:?}"#, e, sql)
 })
}

fn do_parse_tokens(
 input: ParseStream,
 statement_type: ParseStatementType,
) -> syn::Result<proc_macro2::TokenStream> {
 let span = input.span();

 // Get result type and SQL

 let result_type = input.parse::<Type>().ok();
 let sql = input.parse::<LitStr>().ok().map(|s| s.value());

 // Try validating SQL as-is

 let stmt_info = sql.clone().and_then(|s| validate_sql(s).ok());

 // Try adding SELECT if it didn't validate

 let (sql, stmt_info) = match (sql, stmt_info) {
  (Some(sql), None) => {
   let sql_with_select = format!("SELECT {}", sql);
   let stmt_info = validate_sql(&sql_with_select).ok();
   (Some(if stmt_info.is_some() { sql_with_select } else { sql }), stmt_info)
  }
  t => t,
 };

 eprintln!("{:?}, {:?}, {:?}", quote!(#result_type).to_string(), sql, stmt_info);

 // Extract container type (e.g. Vec, Option) if present

 let result_type = match result_type {
  Some(syn::Type::Path(syn::TypePath { path: syn::Path { segments, .. }, .. }))
   if segments.len() == 1 =>
  {
   let segment = segments.first().unwrap();
   Some(match segment.ident.to_string().as_str() {
    "Vec" | "Option" => match &segment.arguments {
     syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. })
      if args.len() == 1 =>
     {
      let arg = args.first().unwrap();
      match arg {
       syn::GenericArgument::Type(syn::Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
       }))
        if segments.len() == 1 =>
       {
        let contents_segment = segments.first().unwrap();
        ResultType {
         container: Some(segment.ident.clone()),
         contents: Some(contents_segment.ident.clone()),
        }
       }
       syn::GenericArgument::Type(syn::Type::Infer(_)) => {
        ResultType { container: Some(segment.ident.clone()), contents: None }
       }
       _ => abort_call_site!("No segments found for container type {:#?}", arg),
      }
     }
     _ => abort_call_site!("No arguments found for container type"),
    },
    _ => ResultType { container: None, contents: Some(segment.ident.clone()) },
   })
  }
  Some(_) => abort_call_site!("Could not parse result_type"),
  None => None,
 };

 eprintln!("{:?}, {:?}, {:?}", result_type, sql, stmt_info);

 // If it didn't still validate and we have a non-inferred result type, try adding SELECT ... FROM

 let (sql, stmt_info) = match (result_type.clone(), sql, stmt_info) {
  //
  // Have result type and SQL did not validate, try generating SELECT ... FROM
  (Some(ResultType { contents: Some(contents), .. }), sql, None) => {
   let result_type = contents.to_string();
   let table_name = result_type.to_lowercase();
   let tables = TABLES.lock().unwrap();
   let table = tables.get(&table_name).unwrap_or_else(|| {
    abort!(
     span,
     "Table {:?} not found. Does struct {} exist and have #[derive(Turbosql)]?",
     table_name,
     result_type
    )
   });

   let column_names_str =
    table.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ");

   let sql = format!("SELECT {} FROM {} {}", column_names_str, table_name, sql.unwrap_or_default());

   (sql.clone(), validate_sql_or_abort(sql))
  }

  // Otherwise, everything is validated, just unwrap
  (_, Some(sql), Some(stmt_info)) => (sql, stmt_info),

  _ => abort_call_site!("no predicate and no result type found"),
 };

 eprintln!("{:?} {:?}, {:?}", &result_type, sql, stmt_info);

 // try parse sql here with nom-sql

 eprintln!("NOM_SQL: {:#?}", nom_sql::parser::parse_query(&sql));

 // pull explicit members from statement info

 // let explicit_members = extract_explicit_members(&stmt_info.column_names);

 // get query params and validate their count against what the statement is expecting

 let QueryParams { params } = input.parse()?;

 if params.len() != stmt_info.parameter_count {
  abort!(
   span,
   "Expected {} bound parameter{}, got {}: {:?}",
   stmt_info.parameter_count,
   if stmt_info.parameter_count == 1 { "" } else { "s" },
   params.len(),
   sql
  );
 }

 if !input.is_empty() {
  return Err(input.error("Expected parameters"));
 }

 // if we return no columns, this should be an execute

 if stmt_info.column_names.is_empty() {
  if !matches!(statement_type, Execute) {
   abort_call_site!("No rows returned from SQL, use execute! instead.");
  }

  return Ok(quote! {
  {
   (|| -> Result<_, _> {
    let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
    let mut stmt = db.prepare_cached(#sql)?;
    stmt.execute(::turbosql::params![#params])
   })()
  }
  });
 }

 if !matches!(statement_type, Select) {
  abort_call_site!("Rows returned from SQL, use select! instead.");
 }

 // dispatch

 // let (struct_members, row_casters) = match (&result_type, &stmt_info, explicit_members) {
 //  (Some(_result_type), stmt_info, None) => {
 //   let members: Vec<_> = stmt_info
 //    .column_names
 //    .iter()
 //    .enumerate()
 //    .map(|(i, col_name)| (format_ident!("{}", col_name), format_ident!("None"), i))
 //    .collect();

 //   let m = MembersAndCasters::create(members);

 //   (m.struct_members, m.row_casters)
 //  }

 //  _ => abort!(span, "Expected explicitly typed return values or a return type."),
 // };

 // let struct_decl = None;
 // let (result_type, struct_decl) = match &result_type {
 //  Some(result_type) => (result_type, None),
 //  // Some(ResultType { contents, .. }) => (quote!(#contents), None),
 //  // Some(t) => (quote!(#t), None, Some(quote!(, ..Default::default()))),
 //  None => {
 //   let tsr = format_ident!("TurbosqlResult");
 //   (
 //    quote!(#tsr),
 //    Some(quote! {
 //     #[derive(Debug, Clone, ::turbosql::Serialize)]
 //     struct #tsr { #(#struct_members),* }
 //    }),
 //   )
 //  }
 // };

 let tokens = match result_type {
  //
  // Vec
  Some(ResultType { container: Some(container), contents: Some(contents) })
   if container == "Vec" =>
  {
   let m = stmt_info
    .membersandcasters()
    .unwrap_or_else(|_| abort_call_site!("stmt_info.membersandcasters failed"));
   let row_casters = m.row_casters;

   quote! {
    {
     // #struct_decl
     (|| -> Result<Vec<#contents>, ::turbosql::Error> {
      let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
      let mut stmt = db.prepare_cached(#sql)?;
      let result = stmt.query_map(::turbosql::params![#params], |row| {
       Ok(#contents {
        #(#row_casters),*
        // #default
       })
      })?.collect::<Vec<_>>();

      let result = result.into_iter().flatten().collect::<Vec<_>>();

      Ok(result)
     })()
    }
   }
  }

  // Option
  Some(ResultType { container: Some(container), contents: Some(contents) })
   if container == "Option" =>
  {
   let m = stmt_info
    .membersandcasters()
    .unwrap_or_else(|_| abort_call_site!("stmt_info.membersandcasters failed"));
   let row_casters = m.row_casters;

   quote! {
    {
     // #struct_decl
     (|| -> Result<Option<#contents>, ::turbosql::Error> {
      use ::turbosql::OptionalExtension;

      let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
      let mut stmt = db.prepare_cached(#sql)?;
      let result = stmt.query_row(::turbosql::params![#params], |row| -> Result<#contents, _> {
       Ok(#contents {
        #(#row_casters),*
        // #default
       })
      }).optional()?;

      Ok(result)
     })()
    }
   }
  }

  // Primitive type
  Some(ResultType { container: None, contents: Some(contents) })
   if ["i64", "bool"].contains(&&contents.to_string().as_str()) =>
  {
   quote! {
    {
     (|| -> Result<#contents, ::turbosql::Error> {
      let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
      let mut stmt = db.prepare_cached(#sql)?;
      let result = stmt.query_row(::turbosql::params![#params], |row| -> Result<#contents, _> {
       Ok(row.get(0)?)
      })?;
      Ok(result)
     })()
    }
   }
  }

  // Custom struct type
  Some(ResultType { container: None, contents: Some(contents) }) => {
   let m = stmt_info
    .membersandcasters()
    .unwrap_or_else(|_| abort_call_site!("stmt_info.membersandcasters failed"));
   let row_casters = m.row_casters;

   quote! {
    {
     (|| -> Result<#contents, ::turbosql::Error> {
      let db = ::turbosql::__TURBOSQL_DB.lock().unwrap();
      let mut stmt = db.prepare_cached(#sql)?;
      let result = stmt.query_row(::turbosql::params![#params], |row| -> Result<#contents, _> {
       Ok(#contents {
        #(#row_casters),*
        // #default
       })
      })?;
      Ok(result)
     })()
    }
   }
  }

  // Inferred
  Some(ResultType { container: Some(container), contents: None }) => abort_call_site!("INFERRED"),
  _ => abort_call_site!("unknown result_type"),
 };

 Ok(tokens)
}

impl Parse for SelectTokens {
 fn parse(input: ParseStream) -> syn::Result<Self> {
  Ok(SelectTokens { tokens: do_parse_tokens(input, Select)? })
 }
}

impl Parse for ExecuteTokens {
 fn parse(input: ParseStream) -> syn::Result<Self> {
  Ok(ExecuteTokens { tokens: do_parse_tokens(input, Execute)? })
 }
}

/// Executes a SQL statement.
#[proc_macro]
#[proc_macro_error]
pub fn execute(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
 let ExecuteTokens { tokens } = parse_macro_input!(input);
 proc_macro::TokenStream::from(tokens)
}

/// Executes a SQL SELECT statement with automatic `SELECT` and `FROM` clauses.
#[proc_macro]
#[proc_macro_error]
pub fn select(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
 let SelectTokens { tokens } = parse_macro_input!(input);
 proc_macro::TokenStream::from(tokens)
}

/// Derive this on a `struct` to create a corresponding SQLite table and `insert`/`update`/`upsert` methods. (TODO: `Turbosql` trait?)
#[proc_macro_derive(Turbosql, attributes(turbosql))]
#[proc_macro_error]
pub fn turbosql_derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
 // parse tokenstream and set up table struct

 let input = parse_macro_input!(input as DeriveInput);
 let table_span = input.span();
 let table_ident = input.ident;
 let table_name = table_ident.to_string().to_lowercase();

 let ltn = LAST_TABLE_NAME.lock().unwrap().clone();

 let mut last_table_name_ref = LAST_TABLE_NAME.lock().unwrap();
 *last_table_name_ref = format!("{}, {}", ltn, table_name);

 let fields = match input.data {
  Data::Struct(ref data) => match data.fields {
   Fields::Named(ref fields) => fields,
   Fields::Unnamed(_) | Fields::Unit => unimplemented!(),
  },
  Data::Enum(_) | Data::Union(_) => unimplemented!(),
 };

 let table = Table {
  ident: table_ident,
  span: table_span,
  name: table_name.clone(),
  columns: extract_columns(fields),
 };

 let minitable = MiniTable {
  name: table_name.clone(),
  columns: table
   .columns
   .iter()
   .map(|c| MiniColumn {
    name: c.name.clone(),
    sql_type: c.sql_type,
    rust_type: c.rust_type.clone(),
   })
   .collect(),
 };

 TABLES.lock().unwrap().insert(table_name, minitable);

 // create trait functions

 let fn_create = create::create(&table);
 let fn_insert = insert::insert(&table);
 let fn_select = select::select(&table);

 // output tokenstream

 proc_macro::TokenStream::from(quote! {
  impl #table {
   #fn_create
   #fn_insert
   #fn_select
  }
 })
}

/// Convert syn::FieldsNamed to our Column type.
fn extract_columns(fields: &FieldsNamed) -> Vec<Column> {
 let columns = fields
  .named
  .iter()
  .filter_map(|f| {
   // Skip (skip) fields

   for attr in &f.attrs {
    let meta = attr.parse_meta().unwrap();
    match meta {
     Meta::List(list) if list.path.is_ident("turbosql") => {
      for value in list.nested.iter() {
       if let NestedMeta::Meta(meta) = value {
        match meta {
         Meta::Path(p) if p.is_ident("skip") => {
          // TODO: For skipped fields, Handle derive(Default) requirement better
          // require Option and manifest None values
          return None;
         }
         _ => (),
        }
       }
      }
     }
     _ => (),
    }
   }

   let ident = &f.ident;
   let name = ident.as_ref().unwrap().to_string();

   let ty = &f.ty;
   let ty_str = quote!(#ty).to_string();

   // TODO: have specific error messages or advice for other numeric types
   // specifically, sqlite cannot represent u64 integers, would be coerced to float.
   // https://sqlite.org/fileformat.html

   let sql_type = match (name.as_str(), ty_str.as_str()) {
    ("rowid", "Option < i64 >") => "INTEGER PRIMARY KEY",
    // (_, "i64") => "INTEGER NOT NULL",
    (_, "Option < i8 >") => "INTEGER",
    (_, "Option < u8 >") => "INTEGER",
    (_, "Option < i16 >") => "INTEGER",
    (_, "Option < u16 >") => "INTEGER",
    (_, "Option < i32 >") => "INTEGER",
    (_, "Option < u32 >") => "INTEGER",
    (_, "Option < i53 >") => "INTEGER",
    (_, "Option < i64 >") => "INTEGER",
    (_, "u64") => abort!(ty, SQLITE_64BIT_ERROR),
    (_, "Option < u64 >") => abort!(ty, SQLITE_64BIT_ERROR),
    // (_, "f64") => "REAL NOT NULL",
    (_, "Option < f64 >") => "REAL",
    // (_, "bool") => "BOOLEAN NOT NULL",
    (_, "Option < bool >") => "BOOLEAN",
    // (_, "String") => "TEXT NOT NULL",
    (_, "Option < String >") => "TEXT",
    // SELECT LENGTH(blob_column) ... will be null if blob is null
    // (_, "Blob") => "BLOB NOT NULL",
    (_, "Option < Blob >") => "BLOB",
    _ => abort!(ty, "turbosql doesn't support rust type: {}", ty_str),
   };

   Some(Column {
    ident: ident.clone().unwrap(),
    span: ty.span(),
    rust_type: ty_str,
    name,
    sql_type,
   })
  })
  .collect::<Vec<_>>();

 // Make sure we have a rowid column, to keep a persistent rowid for blob access.
 // see https://www.sqlite.org/rowidtable.html :
 // "If the rowid is not aliased by INTEGER PRIMARY KEY then it is not persistent and might change."

 if !matches!(
  columns.iter().find(|c| c.name == "rowid"),
  Some(Column { sql_type: "INTEGER PRIMARY KEY", .. })
 ) {
  abort_call_site!("derive(Turbosql) structs must include a 'rowid: Option<i64>' field")
 };

 columns
}
