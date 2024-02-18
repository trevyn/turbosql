#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(uncommon_codepoints)]
#![doc = include_str!("../README.md")]

#[cfg(all(not(feature = "test"), any(test, doctest)))]
compile_error!("turbosql must be tested with '--features test -- --test-threads=1'");

#[cfg(not(target_arch = "wasm32"))]
include!("lib_inner.rs");

#[cfg(target_arch = "wasm32")]
pub use turbosql_impl::{execute, select, update, Turbosql};

#[cfg(target_arch = "wasm32")]
pub fn now_ms() -> i64 {
	panic!("now_ms() is not implemented for wasm32");
}
