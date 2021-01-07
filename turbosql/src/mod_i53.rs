use crate as turbosql;

use self::turbosql::{FromSql, FromSqlResult, ToSql, ValueRef};

use juniper::{ParseScalarResult, ParseScalarValue, Value};
use serde::{Deserialize, Serialize};
use ux::i53 as ux_i53;

impl std::str::FromStr for i53 {
 type Err = String;
 fn from_str(_value: &str) -> Result<Self, Self::Err> {
  unimplemented!()
 }
}

// And we define how to represent i53 as a string.
impl std::fmt::Display for i53 {
 fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
  write!(f, "{}", self.0)
 }
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug)]
#[allow(non_camel_case_types)]
pub struct i53(ux_i53);

#[juniper::graphql_scalar(
 description = "i53: 53-bit signed integer; represented as `i53`/`i64` in Rust, `Float` in GraphQL, `number` in TypeScript."
)]
impl<S> GraphQLScalar for i53
where
 S: ScalarValue,
{
 // Define how to convert your custom scalar into a primitive type.
 fn resolve(&self) -> Value {
  let val: i64 = self.0.into();
  Value::scalar(val as f64)
 }

 // Define how to parse a primitive type into your custom scalar.
 fn from_input_value(v: &InputValue) -> Option<i53> {
  v.as_scalar_value().and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
 }

 // Define how to parse a string value.
 fn from_str<'a>(value: ScalarToken<'a>) -> ParseScalarResult<'a, S> {
  <String as ParseScalarValue<S>>::from_str(value)
 }
}

// impl Deref for i53 {
//  type Target = ux_i53;
//  fn deref(&self) -> &Self::Target {
//   &self.0
//  }
// }

// struct DerefExample<T> {
//     value: T
// }

// impl<T> Deref for DerefExample<T> {
//     type Target = T;

//     fn deref(&self) -> &Self::Target {
//         &self.value
//     }
// }

impl From<i64> for i53 {
 fn from(item: i64) -> Self {
  i53(ux_i53::new(item))
 }
}

impl i53 {
 pub fn as_i64(&self) -> i64 {
  self.0.into()
 }
}

impl FromSql for i53 {
 fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
  Ok(value.as_i64()?.into())
 }
}

impl ToSql for i53 {
 fn to_sql(&self) -> turbosql::Result<turbosql::ToSqlOutput<'_>> {
  Ok(turbosql::ToSqlOutput::Owned(turbosql::Value::Integer(self.0.into())))
 }
}
