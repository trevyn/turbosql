//! This crate provides Turbosql's procedural macros.
//!
//! Please refer to the `turbosql` crate for how to set this up.

#![forbid(unsafe_code)]

const SQLITE_U64_ERROR: &str = r##"SQLite cannot natively store unsigned 64-bit integers, so Turbosql does not support u64 fields. Use i64, u32, f64, or a string or binary format instead. (see https://github.com/trevyn/turbosql/issues/3 )"##;

use once_cell::sync::Lazy;
use proc_macro2::Span;
use proc_macro_error::{abort, abort_call_site, proc_macro_error};
use quote::{format_ident, quote, ToTokens};
use rusqlite::{params, Connection, Statement};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

mod insert;
mod update;

#[derive(Debug, Clone)]
struct Table {
 ident: Ident,
 span: Span,
 name: String,
 columns: Vec<Column>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
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

#[derive(Clone, Serialize, Deserialize, Debug)]
struct MiniColumn {
 name: String,
 rust_type: String,
 sql_type: String,
}

static U8_ARRAY_RE: Lazy<regex::Regex> =
 Lazy::new(|| regex::Regex::new(r"^Option < \[u8 ; \d+\] >$").unwrap());

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
 //  members: Vec<(Ident, Ident, usize)>,
 //  struct_members: Vec<proc_macro2::TokenStream>,
 row_casters: Vec<proc_macro2::TokenStream>,
}

impl MembersAndCasters {
 fn create(members: Vec<(Ident, Ident, usize)>) -> MembersAndCasters {
  // let struct_members: Vec<_> = members.iter().map(|(name, ty, _i)| quote!(#name: #ty)).collect();
  let row_casters = members
   .iter()
   .map(|(name, _ty, i)| {
    if name.to_string().ends_with("__serialized") {
     let name = name.to_string();
     let real_name = format_ident!("{}", name.strip_suffix("__serialized").unwrap());
     quote!(#real_name: {
      let string: String = row.get(#i)?;
      ::turbosql::serde_json::from_str(&string)?
     })
    } else {
     quote!(#name: row.get(#i)?)
    }
   })
   .collect::<Vec<_>>();

  Self { row_casters }
 }
}

fn _extract_explicit_members(columns: &[String]) -> Option<MembersAndCasters> {
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

fn _extract_stmt_members(stmt: &Statement, span: &Span) -> MembersAndCasters {
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
 positional_parameter_count: usize,
 named_parameters: Vec<String>,
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
 output_generated_schema_for_your_information_do_not_edit: Option<String>,
 output_generated_tables_do_not_edit: Option<BTreeMap<String, MiniTable>>,
}

fn migrations_to_tempdb(migrations: &[String]) -> Connection {
 let tempdb = rusqlite::Connection::open_in_memory().unwrap();

 tempdb
  .execute_batch(
   "CREATE TABLE _turbosql_migrations (rowid INTEGER PRIMARY KEY, migration TEXT NOT NULL) STRICT;",
  )
  .unwrap();

 migrations.iter().filter(|m| !m.starts_with("--")).for_each(|m| {
  match tempdb.execute(m, params![]) {
   Ok(_) => (),
   Err(rusqlite::Error::ExecuteReturnedResults) => (), // pragmas
   Err(e) => abort_call_site!("Running migrations on temp db: {:?} {:?}", m, e),
  }
 });

 tempdb
}

fn migrations_to_schema(migrations: &[String]) -> Result<String, rusqlite::Error> {
 Ok(
  migrations_to_tempdb(migrations)
   .prepare("SELECT sql FROM sqlite_master WHERE type='table' ORDER BY sql")?
   .query_map(params![], |row| row.get(0))?
   .collect::<Result<Vec<String>, _>>()?
   .join("\n"),
 )
}

fn read_migrations_toml() -> MigrationsToml {
 let lockfile = std::fs::File::create(std::env::temp_dir().join("migrations.toml.lock")).unwrap();
 fs2::FileExt::lock_exclusive(&lockfile).unwrap();

 let migrations_toml_path = migrations_toml_path();
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

 let stmt = tempdb.prepare(sql.as_ref())?;
 let mut positional_parameter_count = stmt.parameter_count();
 let mut named_parameters = Vec::new();

 for idx in 1..=stmt.parameter_count() {
  if let Some(parameter_name) = stmt.parameter_name(idx) {
   named_parameters.push(parameter_name.to_string());
   positional_parameter_count -= 1;
  }
 }

 Ok(StatementInfo {
  positional_parameter_count,
  named_parameters,
  column_names: stmt.column_names().into_iter().map(str::to_string).collect(),
 })
}

fn validate_sql_or_abort<S: AsRef<str> + std::fmt::Debug>(sql: S) -> StatementInfo {
 validate_sql(sql.as_ref()).unwrap_or_else(|e| {
  abort_call_site!(r#"Error validating SQL statement: "{}". SQL: {:?}"#, e, sql)
 })
}

fn parse_interpolated_sql(
 input: ParseStream,
) -> syn::Result<(Option<String>, Punctuated<Expr, Token![,]>)> {
 let mut params: Punctuated<Expr, Token![,]> = Punctuated::new();

 if input.is_empty() {
  return Ok((None, params));
 }

 let mut sql = input.parse::<LitStr>()?.value();

 if input.parse::<Token![,]>().is_ok() {
  return Ok((Some(sql), input.parse_terminated(Expr::parse)?));
 }

 loop {
  if input.is_empty() {
   break;
  }

  params.push(input.parse()?);
  sql.push('?');

  if input.is_empty() {
   break;
  }

  sql.push_str(&input.parse::<LitStr>()?.value());
 }

 Ok((Some(sql), params))
}

fn do_parse_tokens(
 input: ParseStream,
 statement_type: ParseStatementType,
) -> syn::Result<proc_macro2::TokenStream> {
 let span = input.span();

 // Get result type and SQL

 let result_type = input.parse::<Type>().ok();
 let (sql, params) = parse_interpolated_sql(input)?;

 if std::env::current_exe().unwrap().file_stem().unwrap() == "rust-analyzer" {
  if let Some(ty) = result_type {
   return Ok(quote!(Ok(#ty::default())));
  } else {
   return Ok(quote!());
  }
 }

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

 // eprintln!("{:?}, {:?}, {:?}", quote!(#result_type).to_string(), sql, stmt_info);

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

 // eprintln!("{:?}, {:?}, {:?}", result_type, sql, stmt_info);

 // If it didn't still validate and we have a non-inferred result type, try adding SELECT ... FROM

 let (sql, stmt_info) = match (result_type.clone(), sql, stmt_info) {
  //
  // Have result type and SQL did not validate, try generating SELECT ... FROM
  (Some(ResultType { contents: Some(contents), .. }), sql, None) => {
   let result_type = contents.to_string();
   let table_name = result_type.to_lowercase();
   // let tables = TABLES.lock().unwrap();
   // let table = match tables.get(&table_name) {
   //  Some(t) => t.clone(),
   //  None =>

   let table = {
    let t = match read_migrations_toml().output_generated_tables_do_not_edit {
     Some(m) => m.get(&table_name).cloned(),
     None => None,
    };

    match t {
     Some(t) => t,
     None => {
      abort!(
       span,
       "Table {:?} not found. Does struct {} exist and have #[derive(Turbosql, Default)]?",
       table_name,
       result_type
      );
     }
    }
   };
   // };

   let column_names_str = table
    .columns
    .iter()
    .map(|c| {
     if c.sql_type == "TEXT" && c.rust_type != "Option < String >" {
      format!("{} AS {}__serialized", c.name, c.name)
     } else {
      c.name.clone()
     }
    })
    .collect::<Vec<_>>()
    .join(", ");

   let sql = format!("SELECT {} FROM {} {}", column_names_str, table_name, sql.unwrap_or_default());

   (sql.clone(), validate_sql_or_abort(sql))
  }

  // Otherwise, everything is validated, just unwrap
  (_, Some(sql), Some(stmt_info)) => (sql, stmt_info),

  _ => abort_call_site!("no predicate and no result type found"),
 };

 // eprintln!("{:?} {:?}, {:?}", &result_type, sql, stmt_info);

 // try parse sql here with nom-sql

 // eprintln!("NOM_SQL: {:#?}", nom_sql::parser::parse_query(&sql));

 // pull explicit members from statement info

 // let explicit_members = extract_explicit_members(&stmt_info.column_names);

 // get query params and validate their count against what the statement is expecting

 if !stmt_info.named_parameters.is_empty() {
  abort_call_site!("SQLite named parameters not currently supported.");
 }

 if params.len() != stmt_info.positional_parameter_count {
  #[cfg(test)]
  todo_or_die::issue_closed!("rust-lang", "rust", 54725); // span.join()

  abort_call_site!(
   "Expected {} bound parameter{}, got {}: {:?}",
   stmt_info.positional_parameter_count,
   if stmt_info.positional_parameter_count == 1 { "" } else { "s" },
   params.len(),
   sql
  );
 }

 if !input.is_empty() {
  return Err(input.error("Expected parameters"));
 }

 let params = if stmt_info.named_parameters.is_empty() {
  quote! { ::turbosql::params![#params] }
 } else {
  let param_quotes = stmt_info.named_parameters.iter().map(|p| {
   let var_ident = format_ident!("{}", &p[1..]);
   quote!(#p: &#var_ident,)
  });
  quote! { ::turbosql::named_params![#(#param_quotes),*] }
 };

 // if we return no columns, this should be an execute

 if stmt_info.column_names.is_empty() {
  if !matches!(statement_type, Execute) {
   abort_call_site!("No rows returned from SQL, use execute! instead.");
  }

  return Ok(quote! {
  {
   (|| -> Result<usize, ::turbosql::Error> {
    ::turbosql::__TURBOSQL_DB.with(|db| {
     let db = db.borrow_mut();
     let mut stmt = db.prepare_cached(#sql)?;
     Ok(stmt.execute(#params)?)
    })
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
  // Vec of primitive type
  Some(ResultType { container: Some(container), contents: Some(contents) })
   if container == "Vec"
    && ["f32", "f64", "i8", "u8", "i16", "u16", "i32", "u32", "i64", "String", "bool"]
     .contains(&contents.to_string().as_str()) =>
  {
   quote! {
    {
     (|| -> Result<Vec<#contents>, ::turbosql::Error> {
      ::turbosql::__TURBOSQL_DB.with(|db| {
       let db = db.borrow_mut();
       let mut stmt = db.prepare_cached(#sql)?;
       let result = stmt.query_and_then(#params, |row| -> Result<#contents, ::turbosql::Error> {
        Ok(row.get(0)?)
       })?.collect::<Vec<_>>();
       let result = result.into_iter().flatten().collect::<Vec<_>>();
       Ok(result)
      })
     })()
    }
   }
  }

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
      ::turbosql::__TURBOSQL_DB.with(|db| {
       let db = db.borrow_mut();
       let mut stmt = db.prepare_cached(#sql)?;
       let result = stmt.query_and_then(#params, |row| -> Result<#contents, ::turbosql::Error> {
        Ok(#contents {
         #(#row_casters),*
         // #default
        })
       })?.collect::<Vec<_>>();
       let result = result.into_iter().flatten().collect::<Vec<_>>();
       Ok(result)
      })
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
      ::turbosql::__TURBOSQL_DB.with(|db| {
       let db = db.borrow_mut();
       let mut stmt = db.prepare_cached(#sql)?;
       let mut rows = stmt.query(#params)?;
       Ok(if let Some(row) = rows.next()? {
        Some(#contents {
         #(#row_casters),*
        })
       }
       else {
        None
       })
      })
     })()
    }
   }
  }

  // Primitive type
  Some(ResultType { container: None, contents: Some(contents) })
   if ["f32", "f64", "i8", "u8", "i16", "u16", "i32", "u32", "i64", "String", "bool"]
    .contains(&contents.to_string().as_str()) =>
  {
   quote! {
    {
     (|| -> Result<#contents, ::turbosql::Error> {
      ::turbosql::__TURBOSQL_DB.with(|db| {
       let db = db.borrow_mut();
       let mut stmt = db.prepare_cached(#sql)?;
       let result = stmt.query_row(#params, |row| {
        row.get(0)
       })?;
       Ok(result)
      })
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
      ::turbosql::__TURBOSQL_DB.with(|db| {
       let db = db.borrow_mut();
       let mut stmt = db.prepare_cached(#sql)?;
       let mut rows = stmt.query(#params)?;
       let row = rows.next()?.ok_or(::turbosql::rusqlite::Error::QueryReturnedNoRows)?;
       Ok(#contents {
        #(#row_casters),*
       })
      })
     })()
    }
   }
  }

  // Inferred
  Some(ResultType { container: Some(_container), contents: None }) => abort_call_site!("INFERRED"),
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

/// Executes a SQL statement. On success, returns the number of rows that were changed or inserted or deleted.
#[proc_macro]
#[proc_macro_error]
pub fn execute(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
 let ExecuteTokens { tokens } = parse_macro_input!(input);
 proc_macro::TokenStream::from(tokens)
}

/// Executes a SQL SELECT statement with optionally automatic `SELECT` and `FROM` clauses.
#[proc_macro]
#[proc_macro_error]
pub fn select(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
 let SelectTokens { tokens } = parse_macro_input!(input);
 proc_macro::TokenStream::from(tokens)
}

/// Derive this on a `struct` to create a corresponding SQLite table and `Turbosql` trait methods.
#[proc_macro_derive(Turbosql, attributes(turbosql))]
#[proc_macro_error]
pub fn turbosql_derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
 if std::env::current_exe().unwrap().file_stem().unwrap() == "rust-analyzer" {
  return quote!().into();
 }

 // parse tokenstream and set up table struct

 let input = parse_macro_input!(input as DeriveInput);
 let table_span = input.span();
 let table_ident = input.ident;
 let table_name = table_ident.to_string().to_lowercase();

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
  name: table_name,
  columns: table
   .columns
   .iter()
   .map(|c| MiniColumn {
    name: c.name.clone(),
    sql_type: c.sql_type.to_string(),
    rust_type: c.rust_type.clone(),
   })
   .collect(),
 };

 create(&table, &minitable);

 // create trait functions

 let fn_insert = insert::insert(&table);
 let fn_update = update::update(&table);

 // output tokenstream

 let output = quote! {
  #[cfg(not(target_arch = "wasm32"))]
  impl ::turbosql::Turbosql for #table {
   #fn_insert
   #fn_update
  }
 };

 output.into()
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

   let sql_type = match (
    name.as_str(),
    if U8_ARRAY_RE.is_match(&ty_str) { "Option < [u8; _] >" } else { ty_str.as_str() },
   ) {
    ("rowid", "Option < i64 >") => "INTEGER PRIMARY KEY",
    (_, "Option < i8 >") => "INTEGER",
    (_, "Option < u8 >") => "INTEGER",
    (_, "Option < i16 >") => "INTEGER",
    (_, "Option < u16 >") => "INTEGER",
    (_, "Option < i32 >") => "INTEGER",
    (_, "Option < u32 >") => "INTEGER",
    (_, "Option < i64 >") => "INTEGER",
    (_, "Option < u64 >") => abort!(ty, SQLITE_U64_ERROR),
    (_, "Option < f64 >") => "REAL",
    (_, "Option < f32 >") => "REAL",
    (_, "Option < bool >") => "INTEGER",
    (_, "Option < String >") => "TEXT",
    // SELECT LENGTH(blob_column) ... will be null if blob is null
    (_, "Option < Blob >") => "BLOB",
    (_, "Option < Vec < u8 > >") => "BLOB",
    (_, "Option < [u8; _] >") => "BLOB",
    _ => {
     if ty_str.starts_with("Option < ") {
      "TEXT" // JSON-serialized
     } else {
      abort!(
       ty,
       "Turbosql types must be wrapped in Option for forward/backward schema compatibility. Try: Option<{}>",
       ty_str
      )
     }
    }
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

use std::fs;

/// CREATE TABLE
fn create(table: &Table, minitable: &MiniTable) {
 // create the migrations

 let sql = makesql_create(table);

 rusqlite::Connection::open_in_memory().unwrap().execute(&sql, params![]).unwrap_or_else(|e| {
  abort_call_site!("Error validating auto-generated CREATE TABLE statement: {} {:#?}", sql, e)
 });

 let target_migrations = make_migrations(table);

 // read in the existing migrations from toml

 let lockfile = std::fs::File::create(std::env::temp_dir().join("migrations.toml.lock")).unwrap();
 fs2::FileExt::lock_exclusive(&lockfile).unwrap();

 let migrations_toml_path = migrations_toml_path();
 let migrations_toml_path_lossy = migrations_toml_path.to_string_lossy();

 let old_toml_str = if migrations_toml_path.exists() {
  fs::read_to_string(&migrations_toml_path)
   .unwrap_or_else(|e| abort_call_site!("Unable to read {}: {:?}", migrations_toml_path_lossy, e))
 } else {
  String::new()
 };

 let source_migrations_toml: MigrationsToml = toml::from_str(&old_toml_str).unwrap_or_else(|e| {
  abort_call_site!("Unable to decode toml in {}: {:?}", migrations_toml_path_lossy, e)
 });

 // add any migrations that aren't already present

 let mut output_migrations = source_migrations_toml.migrations_append_only.unwrap_or_default();

 #[allow(clippy::search_is_some)]
 target_migrations.iter().for_each(|target_m| {
  if output_migrations
   .iter()
   .find(|source_m| (source_m == &target_m) || (source_m == &&format!("--{}", target_m)))
   .is_none()
  {
   output_migrations.push(target_m.clone());
  }
 });

 let mut tables = source_migrations_toml.output_generated_tables_do_not_edit.unwrap_or_default();
 tables.insert(table.name.clone(), minitable.clone());

 // save to toml

 let mut new_toml_str = String::new();
 let mut serializer = toml::Serializer::pretty(&mut new_toml_str);
 serializer.pretty_array_indent(2);

 MigrationsToml {
  output_generated_schema_for_your_information_do_not_edit: Some(format!(
   "  {}\n",
   migrations_to_schema(&output_migrations)
    .unwrap()
    .replace('\n', "\n  ")
    .replace('(', "(\n    ")
    .replace(", ", ",\n    ")
    .replace(')', "\n  )")
  )),
  migrations_append_only: Some(output_migrations),
  output_generated_tables_do_not_edit: Some(tables),
 }
 .serialize(&mut serializer)
 .unwrap_or_else(|e| abort_call_site!("Unable to serialize migrations toml: {:?}", e));

 let new_toml_str = format!("# This file is auto-generated by Turbosql.\n# It is used to create and apply automatic schema migrations.\n# It should be checked into source control.\n# Modifying it by hand may be dangerous; see the docs.\n\n{}", &new_toml_str);

 // Only write migrations.toml file if it has actually changed;
 // this keeps file mod date clean so cargo doesn't pathologically rebuild

 if old_toml_str != new_toml_str {
  fs::write(&migrations_toml_path, new_toml_str)
   .unwrap_or_else(|e| abort_call_site!("Unable to write {}: {:?}", migrations_toml_path_lossy, e));
 }
}

fn makesql_create(table: &Table) -> String {
 format!(
  "CREATE TABLE {} ({}) STRICT",
  table.name,
  table.columns.iter().map(|c| format!("{} {}", c.name, c.sql_type)).collect::<Vec<_>>().join(",")
 )
}

fn make_migrations(table: &Table) -> Vec<String> {
 let mut vec = vec![format!("CREATE TABLE {} (rowid INTEGER PRIMARY KEY) STRICT", table.name)];

 let mut alters = table
  .columns
  .iter()
  .filter_map(|c| match (c.name.as_str(), c.sql_type) {
   ("rowid", "INTEGER PRIMARY KEY") => None,
   _ => Some(format!("ALTER TABLE {} ADD COLUMN {} {}", table.name, c.name, c.sql_type)),
  })
  .collect::<Vec<_>>();

 vec.append(&mut alters);

 vec
}

fn migrations_toml_path() -> std::path::PathBuf {
 let mut path = std::path::PathBuf::from(env!("OUT_DIR"));
 while path.file_name() != Some(std::ffi::OsStr::new("target")) {
  path.pop();
 }
 path.pop();
 path.push(MIGRATIONS_FILENAME);
 path
}
