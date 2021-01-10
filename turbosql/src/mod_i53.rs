use crate as turbosql;

use self::turbosql::{FromSql, FromSqlResult, ToSql, ValueRef};

use anyhow::anyhow;
use juniper::{ParseScalarResult, ParseScalarValue, Value};
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use ux_serde::i53 as ux_i53;

pub const MAX_SAFE_INTEGER: i64 = 9007199254740991;
pub const MIN_SAFE_INTEGER: i64 = -9007199254740991;

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

impl<T> PartialEq<T> for i53
where
 T: TryInto<i53> + Copy + PartialEq + Ord,
{
 fn eq(&self, other: &T) -> bool {
  match (*other).try_into() {
   Ok(v) => v.0 == self.0,
   Err(_) => false,
  }
 }
}

impl PartialEq for i53 {
 fn eq(&self, other: &i53) -> bool {
  self.0 == other.0
 }
}

impl Eq for i53 {}

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
  v.as_float_value()?.try_into().ok()
 }

 // Define how to parse a string value.
 fn from_str<'a>(value: ScalarToken<'a>) -> ParseScalarResult<'a, S> {
  <String as ParseScalarValue<S>>::from_str(value)
 }
}

impl From<i32> for i53 {
 fn from(item: i32) -> Self {
  i53(ux_i53::new(item as i64))
 }
}

impl TryFrom<i64> for i53 {
 type Error = anyhow::Error;
 fn try_from(item: i64) -> Result<Self, Self::Error> {
  let item_i64 = item as i64;
  let item_i53 = i53(ux_i53::new(item_i64));
  if item_i53.as_i64() as i64 != item {
   return Err(anyhow!("i53 conversion failed: {:#?}", item));
  }

  Ok(item_i53)
 }
}

impl TryFrom<usize> for i53 {
 type Error = anyhow::Error;
 fn try_from(item: usize) -> Result<Self, Self::Error> {
  let item_i64 = item as i64;
  let item_i53 = i53(ux_i53::new(item_i64));
  if item_i53.as_i64() as usize != item {
   return Err(anyhow!("i53 conversion failed: {:#?}", item));
  }

  Ok(item_i53)
 }
}

impl TryFrom<f64> for i53 {
 type Error = anyhow::Error;
 fn try_from(item: f64) -> Result<Self, Self::Error> {
  let item_i64 = item as i64;
  let item_i53 = i53(ux_i53::new(item_i64));
  if item_i53.as_i64() as f64 != item {
   return Err(anyhow!("i53 conversion failed: {:#?}", item));
  }

  Ok(item_i53)
 }
}

impl i53 {
 pub fn as_i64(&self) -> i64 {
  self.0.into()
 }
}

impl FromSql for i53 {
 fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
  value.as_i64()?.try_into().or_else(|_| Err(rusqlite::types::FromSqlError::InvalidType))
 }
}

impl ToSql for i53 {
 fn to_sql(&self) -> turbosql::Result<turbosql::ToSqlOutput<'_>> {
  Ok(turbosql::ToSqlOutput::Owned(turbosql::Value::Integer(self.0.into())))
 }
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn test_rectangle() {
  // let mut rectangle = Rectangle::new(4, 5);
  let i: i53 = 20_usize.try_into().unwrap();
  assert_eq!(i, 20_i32)
 }
}
