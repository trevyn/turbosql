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
use syn::{
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
	spanned::Spanned,
	*,
};

#[cfg(not(feature = "test"))]
const MIGRATIONS_FILENAME: &str = "migrations.toml";
#[cfg(feature = "test")]
const MIGRATIONS_FILENAME: &str = "test.migrations.toml";

mod delete;
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
	sql_default: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct MiniColumn {
	name: String,
	rust_type: String,
	sql_type: String,
}

static OPTION_U8_ARRAY_RE: Lazy<regex::Regex> =
	Lazy::new(|| regex::Regex::new(r"^Option < \[u8 ; \d+\] >$").unwrap());
static U8_ARRAY_RE: Lazy<regex::Regex> =
	Lazy::new(|| regex::Regex::new(r"^\[u8 ; \d+\]$").unwrap());

#[derive(Clone, Debug)]
struct SingleColumn {
	table: Ident,
	column: Ident,
}

#[derive(Clone, Debug)]
enum Content {
	Type(Type),
	#[allow(dead_code)]
	SingleColumn(SingleColumn),
}

impl ToTokens for Content {
	fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
		match self {
			Content::Type(ty) => ty.to_tokens(tokens),
			Content::SingleColumn(_) => unimplemented!(),
		}
	}
}

impl Content {
	fn ty(&self) -> Result<Type> {
		match self {
			Content::Type(ty) => Ok(ty.clone()),
			Content::SingleColumn(_) => unimplemented!(), // parse_str(&c.rust_type),
		}
	}
	fn is_primitive(&self) -> bool {
		match self {
			Content::Type(Type::Path(TypePath { path, .. })) => {
				["f32", "f64", "i8", "u8", "i16", "u16", "i32", "u32", "i64", "String", "bool", "Blob"]
					.contains(&path.segments.last().unwrap().ident.to_string().as_str())
			}
			Content::Type(Type::Array(_)) => true,
			_ => abort_call_site!("Unsupported content type in is_primitive {:#?}", self),
		}
	}
	fn table_ident(&self) -> &Ident {
		match self {
			Content::Type(Type::Path(TypePath { path, .. })) => &path.segments.last().unwrap().ident,
			Content::SingleColumn(c) => &c.table,
			_ => abort_call_site!("Unsupported content type in table_ident {:#?}", self),
		}
	}
}

#[derive(Clone, Debug)]
struct ResultType {
	container: Option<Ident>,
	content: Content,
}

impl ResultType {
	fn ty(&self) -> Result<Type> {
		let content = self.content.ty()?;
		match self.container {
			Some(ref container) => Ok(parse_quote!(#container < #content >)),
			None => Ok(content),
		}
	}
}

// impl Parse for ResultType {
//  fn parse(input: ParseStream) -> Result<Self> {
//   let path = input.parse::<Path>();
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
	// parse_str::<Ident>

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

const SELECT: usize = 1;
const EXECUTE: usize = 2;
const UPDATE: usize = 3;

#[derive(Debug)]
struct StatementInfo {
	positional_parameter_count: usize,
	named_parameters: Vec<String>,
	column_names: Vec<String>,
}

impl StatementInfo {
	fn membersandcasters(&self) -> Result<MembersAndCasters> {
		Ok(MembersAndCasters::create(
			self
				.column_names
				.iter()
				.enumerate()
				.map(|(i, col_name)| Ok((parse_str::<Ident>(col_name)?, format_ident!("None"), i)))
				.collect::<Result<Vec<_>>>()?,
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
		.execute_batch(if cfg!(feature = "sqlite-compat-no-strict-tables") {
			"CREATE TABLE _turbosql_migrations (rowid INTEGER PRIMARY KEY, migration TEXT NOT NULL);"
		} else {
			"CREATE TABLE _turbosql_migrations (rowid INTEGER PRIMARY KEY, migration TEXT NOT NULL) STRICT;"
		})
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

fn migrations_to_schema(migrations: &[String]) -> rusqlite::Result<String> {
	Ok(
		migrations_to_tempdb(migrations)
			.prepare("SELECT sql FROM sqlite_master WHERE type='table' ORDER BY sql")?
			.query_map(params![], |row| row.get(0))?
			.collect::<rusqlite::Result<Vec<String>>>()?
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

fn validate_sql<S: AsRef<str>>(sql: S) -> rusqlite::Result<StatementInfo> {
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
) -> Result<(Option<String>, Punctuated<Expr, Token![,]>, proc_macro2::TokenStream)> {
	if input.is_empty() {
		return Ok(Default::default());
	}

	let sql_token = input.parse::<LitStr>()?;
	let mut sql = sql_token.value();

	if let Ok(comma_token) = input.parse::<Token![,]>() {
		let punctuated_tokens = input.parse_terminated(Expr::parse, Token![,])?;
		return Ok((
			Some(sql),
			punctuated_tokens.clone(),
			quote!(#sql_token #comma_token #punctuated_tokens),
		));
	}

	let mut params = Punctuated::new();

	loop {
		while input.peek(LitStr) {
			sql.push(' ');
			sql.push_str(&input.parse::<LitStr>()?.value());
		}

		if input.is_empty() {
			break;
		}

		params.push(input.parse()?);
		sql.push_str(" ? ");
		if input.parse::<Token![,]>().is_ok() {
			sql.push(',');
		}
	}

	Ok((Some(sql), params, Default::default()))
}

fn do_parse_tokens<const T: usize>(input: ParseStream) -> Result<proc_macro2::TokenStream> {
	let span = input.span();

	// Get result type and SQL

	let result_type = input.parse::<Type>().ok().map(|ty| match ty {
		Type::Path(TypePath { qself: None, ref path }) => {
			let path = path.segments.last().unwrap();
			let mut container = None;

			let content = match &path.ident {
				ident if ["Vec", "Option"].contains(&ident.to_string().as_str()) => {
					container = Some(ident.clone());
					match path.arguments {
						PathArguments::AngleBracketed(AngleBracketedGenericArguments { ref args, .. }) => {
							if args.len() != 1 {
								abort!(args, "Expected 1 argument, found {}", args.len());
							}
							let ty = args.first().unwrap();
							match ty {
								GenericArgument::Type(ty) => Content::Type(ty.clone()),
								_ => abort!(ty, "Expected type, found {:?}", ty),
							}
						}
						_ => {
							abort!(path.arguments, "Expected angle bracketed arguments, found {:?}", path.arguments)
						}
					}
				}
				_ => Content::Type(ty),
			};

			ResultType { container, content }
		}
		Type::Array(array) => ResultType { container: None, content: Content::Type(Type::Array(array)) },
		_ => {
			abort!(ty, "Unknown type {:?}", ty);
		}
	});

	let (mut sql, params, sql_and_parameters_tokens) = parse_interpolated_sql(input)?;

	// Try validating SQL as-is

	let mut stmt_info = sql.as_ref().and_then(|s| validate_sql(s).ok());

	// Try adding SELECT or UPDATE if it didn't validate

	if let (true, Some(orig_sql), None) = (T == SELECT || T == UPDATE, &sql, &stmt_info) {
		let sql_modified = format!("{} {}", if T == SELECT { "SELECT" } else { "UPDATE" }, orig_sql);
		if let Ok(stmt_info_modified) = validate_sql(&sql_modified) {
			sql = Some(sql_modified);
			stmt_info = Some(stmt_info_modified);
		}
	}

	if is_rust_analyzer() {
		return Ok(if let Some(ty) = result_type {
			let ty = ty.ty()?;
			quote!(Ok({let x: #ty = Default::default(); x}))
		} else {
			quote!()
		});
	}

	// If it still didn't validate and we have a non-inferred result type, try adding SELECT ... FROM

	let (sql, stmt_info) = match (result_type.clone(), sql, stmt_info) {
		//
		// Have result type and SQL did not validate, try generating SELECT ... FROM
		(Some(ResultType { content, .. }), sql, None) => {
			let table_type = content.table_ident().to_string();
			let table_name = table_type.to_lowercase();
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
							table_type
						);
					}
				}
			};

			let column_names_str = table
				.columns
				.iter()
				.filter_map(|c| {
					if match &content {
						Content::SingleColumn(col) => col.column == c.name,
						_ => true,
					} {
						if c.sql_type.starts_with("TEXT")
							&& c.rust_type != "Option < String >"
							&& c.rust_type != "String"
						{
							Some(format!("{} AS {}__serialized", c.name, c.name))
						} else {
							Some(c.name.clone())
						}
					} else {
						None
					}
				})
				.collect::<Vec<_>>()
				.join(", ");

			let sql = format!("SELECT {} FROM {} {}", column_names_str, table_name, sql.unwrap_or_default());

			(sql.clone(), validate_sql_or_abort(sql))
		}

		// Otherwise, everything is validated, just unwrap
		(_, Some(sql), Some(stmt_info)) => (sql, stmt_info),
		(_, Some(sql), _) => abort_call_site!("sql did not validate: {}", sql),
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
		abort!(
			sql_and_parameters_tokens,
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
		if T == SELECT {
			abort_call_site!("No rows returned from SQL, use execute! instead.");
		}

		return Ok(quote! {
		{
			(|| -> std::result::Result<usize, ::turbosql::Error> {
				::turbosql::__TURBOSQL_DB.with(|db| {
					let db = db.borrow_mut();
					let mut stmt = db.prepare_cached(#sql)?;
					Ok(stmt.execute(#params)?)
				})
			})()
		}
		});
	}

	if T != SELECT {
		abort_call_site!("Rows returned from SQL, use select! instead.");
	}

	// Decide how to handle selected rows depending on content type.

	let Some(ResultType { container, content }) = result_type else {
		abort_call_site!("unknown result_type")
	};

	let handle_row;
	let content_ty;

	if content.is_primitive() {
		handle_row = quote! { row.get(0)? };
		content_ty = quote! { #content };
	} else {
		let MembersAndCasters { row_casters } = stmt_info
			.membersandcasters()
			.unwrap_or_else(|_| abort_call_site!("stmt_info.membersandcasters failed"));

		handle_row = quote! {
			#[allow(clippy::needless_update)]
			#content {
				#(#row_casters),*,
				..Default::default()
			}
		};
		content_ty = quote! { #content };
	}

	// Content::SingleColumn(col) => {
	// 	handle_row = quote! { row.get(0)? };
	// 	let rust_ty: Type = parse_str(
	// 		&read_migrations_toml()
	// 			.output_generated_tables_do_not_edit
	// 			.unwrap()
	// 			.get(&col.table.to_string().to_lowercase())
	// 			.unwrap()
	// 			.columns
	// 			.iter()
	// 			.find(|c| col.column == c.name)
	// 			.unwrap()
	// 			.rust_type,
	// 	)?;
	// 	content_ty = quote! { #rust_ty };
	// }
	// };

	// Decide how to handle the iterator over rows depending on container.

	let return_type;
	let handle_result;

	match container {
		Some(ident) if ident == "Vec" => {
			return_type = quote! { Vec<#content_ty> };
			handle_result = quote! { result.collect::<Vec<_>>() };
		}
		Some(ident) if ident == "Option" => {
			return_type = quote! { Option<#content_ty> };
			handle_result = quote! { result.next() };
		}
		None => {
			return_type = quote! { #content_ty };
			handle_result =
				quote! { result.next().ok_or(::turbosql::rusqlite::Error::QueryReturnedNoRows)? };
		}
		_ => unreachable!("No other container type is possible"),
	}

	// Put it all together

	Ok(quote! {
		{
			(|| -> std::result::Result<#return_type, ::turbosql::Error> {
				::turbosql::__TURBOSQL_DB.with(|db| {
					let db = db.borrow_mut();
					let mut stmt = db.prepare_cached(#sql)?;
					let mut result = stmt.query_and_then(#params, |row| -> std::result::Result<#content_ty, ::turbosql::Error> {
						Ok(#handle_row)
					})?.flatten();
					Ok(#handle_result)
				})
			})()
		}
	})
}

/// Executes a SQL statement. On success, returns the number of rows that were changed or inserted or deleted.
#[proc_macro]
#[proc_macro_error]
pub fn execute(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	parse_macro_input!(input with do_parse_tokens::<EXECUTE>).into()
}

/// Executes a SQL SELECT statement with optionally automatic `SELECT` and `FROM` clauses.
#[proc_macro]
#[proc_macro_error]
pub fn select(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	parse_macro_input!(input with do_parse_tokens::<SELECT>).into()
}

/// Executes a SQL statement with optionally automatic `UPDATE` clause. On success, returns the number of rows that were changed.
#[proc_macro]
#[proc_macro_error]
pub fn update(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	parse_macro_input!(input with do_parse_tokens::<UPDATE>).into()
}

/// Derive this on a `struct` to create a corresponding SQLite table and `Turbosql` trait methods.
#[proc_macro_derive(Turbosql, attributes(turbosql))]
#[proc_macro_error]
pub fn turbosql_derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
	if is_rust_analyzer() {
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
	let fn_delete = delete::delete(&table);

	// output tokenstream

	quote! {
		#[cfg(not(target_arch = "wasm32"))]
		impl ::turbosql::Turbosql for #table {
			#fn_insert
			#fn_update
			#fn_delete
		}
	}
	.into()
}

/// Convert syn::FieldsNamed to our Column type.
fn extract_columns(fields: &FieldsNamed) -> Vec<Column> {
	let columns = fields
		.named
		.iter()
		.filter_map(|f| {
			let mut sql_default = None;

			for attr in &f.attrs {
				if attr.path().is_ident("turbosql") {
					for meta in attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated).unwrap() {
						match &meta {
							Meta::Path(path) if path.is_ident("skip") => {
								return None;
							}
							Meta::NameValue(MetaNameValue { path, value: Expr::Lit(ExprLit { lit, .. }), .. })
								if path.is_ident("sql_default") =>
							{
								match lit {
									Lit::Bool(value) => sql_default = Some(value.value().to_string()),
									Lit::Int(token) => sql_default = Some(token.to_string()),
									Lit::Float(token) => sql_default = Some(token.to_string()),
									Lit::Str(token) => sql_default = Some(format!("'{}'", token.value())),
									Lit::ByteStr(token) => {
										use std::fmt::Write;
										sql_default = Some(format!(
											"x'{}'",
											token.value().iter().fold(String::new(), |mut o, b| {
												let _ = write!(o, "{b:02x}");
												o
											})
										))
									}
									_ => (),
								}
							}
							_ => (),
						}
					}
				}
			}

			let ident = &f.ident;
			let name = ident.as_ref().unwrap().to_string();

			let ty = &f.ty;
			let ty_str = quote!(#ty).to_string();

			let (sql_type, default_example) = match (
				name.as_str(),
				if OPTION_U8_ARRAY_RE.is_match(&ty_str) {
					"Option < [u8; _] >"
				} else if U8_ARRAY_RE.is_match(&ty_str) {
					"[u8; _]"
				} else {
					ty_str.as_str()
				},
			) {
				("rowid", "Option < i64 >") => ("INTEGER PRIMARY KEY", "NULL"),
				(_, "Option < i8 >") => ("INTEGER", "0"),
				(_, "i8") => ("INTEGER NOT NULL", "0"),
				(_, "Option < u8 >") => ("INTEGER", "0"),
				(_, "u8") => ("INTEGER NOT NULL", "0"),
				(_, "Option < i16 >") => ("INTEGER", "0"),
				(_, "i16") => ("INTEGER NOT NULL", "0"),
				(_, "Option < u16 >") => ("INTEGER", "0"),
				(_, "u16") => ("INTEGER NOT NULL", "0"),
				(_, "Option < i32 >") => ("INTEGER", "0"),
				(_, "i32") => ("INTEGER NOT NULL", "0"),
				(_, "Option < u32 >") => ("INTEGER", "0"),
				(_, "u32") => ("INTEGER NOT NULL", "0"),
				(_, "Option < i64 >") => ("INTEGER", "0"),
				(_, "i64") => ("INTEGER NOT NULL", "0"),
				(_, "Option < u64 >") => abort!(ty, SQLITE_U64_ERROR),
				(_, "u64") => abort!(ty, SQLITE_U64_ERROR),
				(_, "Option < f64 >") => ("REAL", "0.0"),
				(_, "f64") => ("REAL NOT NULL", "0.0"),
				(_, "Option < f32 >") => ("REAL", "0.0"),
				(_, "f32") => ("REAL NOT NULL", "0.0"),
				(_, "Option < bool >") => ("INTEGER", "false"),
				(_, "bool") => ("INTEGER NOT NULL", "false"),
				(_, "Option < String >") => ("TEXT", "\"\""),
				(_, "String") => ("TEXT NOT NULL", "''"),
				// SELECT LENGTH(blob_column) ... will be null if blob is null
				(_, "Option < Blob >") => ("BLOB", "b\"\""),
				(_, "Blob") => ("BLOB NOT NULL", "''"),
				(_, "Option < Vec < u8 > >") => ("BLOB", "b\"\""),
				(_, "Vec < u8 >") => ("BLOB NOT NULL", "''"),
				(_, "Option < [u8; _] >") => ("BLOB", "b\"\\x00\\x01\\xff\""),
				(_, "[u8; _]") => ("BLOB NOT NULL", "''"),
				_ => {
					// JSON-serialized
					if ty_str.starts_with("Option < ") {
						("TEXT", "\"\"")
					} else {
						("TEXT NOT NULL", "''")
					}
				}
			};

			if sql_default.is_none() && sql_type.ends_with("NOT NULL") {
				sql_default = Some(default_example.into());
				// abort!(f, "Field `{}` has no default value and is not nullable. Either add a default value with e.g. #[turbosql(sql_default = {default_example})] or make it Option<{ty_str}>.", name);
			}

			Some(Column {
				ident: ident.clone().unwrap(),
				span: ty.span(),
				rust_type: ty_str,
				name,
				sql_type,
				sql_default,
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
	let serializer = toml::Serializer::pretty(&mut new_toml_str);
	// serializer.pretty_array_indent(2);

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
	.serialize(serializer)
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
	let columns =
		table.columns.iter().map(|c| format!("{} {}", c.name, c.sql_type)).collect::<Vec<_>>().join(",");

	if cfg!(feature = "sqlite-compat-no-strict-tables") {
		format!("CREATE TABLE {} ({})", table.name, columns)
	} else {
		format!("CREATE TABLE {} ({}) STRICT", table.name, columns)
	}
}

fn make_migrations(table: &Table) -> Vec<String> {
	let sql = if cfg!(feature = "sqlite-compat-no-strict-tables") {
		format!("CREATE TABLE {} (rowid INTEGER PRIMARY KEY)", table.name)
	} else {
		format!("CREATE TABLE {} (rowid INTEGER PRIMARY KEY) STRICT", table.name)
	};

	let mut vec = vec![sql];

	let mut alters = table
		.columns
		.iter()
		.filter_map(|c| match (c.name.as_str(), c.sql_type, &c.sql_default) {
			("rowid", "INTEGER PRIMARY KEY", _) => None,
			(_, _, None) => Some(format!("ALTER TABLE {} ADD COLUMN {} {}", table.name, c.name, c.sql_type)),
			(_, _, Some(sql_default)) => Some(format!(
				"ALTER TABLE {} ADD COLUMN {} {} DEFAULT {}",
				table.name, c.name, c.sql_type, sql_default
			)),
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

fn is_rust_analyzer() -> bool {
	std::env::current_exe()
		.unwrap()
		.file_stem()
		.unwrap()
		.to_string_lossy()
		.starts_with("rust-analyzer")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_extract_columns() {
		let fields_named = parse_quote!({
			rowid: Option<i64>,
			name: Option<String>,
			age: Option<u32>,
			awesomeness: Option<f64>,
			#[turbosql(skip)]
			skipped: Option<bool>
		});

		let columns = extract_columns(&fields_named);

		assert_eq!(columns.len(), 4);

		assert_eq!(columns[0].name, "rowid");
		assert_eq!(columns[0].rust_type, "Option < i64 >");
		assert_eq!(columns[0].sql_type, "INTEGER PRIMARY KEY");

		assert_eq!(columns[1].name, "name");
		assert_eq!(columns[1].rust_type, "Option < String >");
		assert_eq!(columns[1].sql_type, "TEXT");

		assert_eq!(columns[2].name, "age");
		assert_eq!(columns[2].rust_type, "Option < u32 >");
		assert_eq!(columns[2].sql_type, "INTEGER");

		assert_eq!(columns[3].name, "awesomeness");
		assert_eq!(columns[3].rust_type, "Option < f64 >");
		assert_eq!(columns[3].sql_type, "REAL");

		assert!(!columns.iter().any(|c| c.name == "skipped"));
	}
}
