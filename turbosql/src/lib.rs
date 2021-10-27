#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(all(not(feature = "test"), any(test, doctest)))]
compile_error!("turbosql must be tested with '--features test'");

#[cfg(not(target_arch = "wasm32"))]
include!("lib_inner.rs");

#[cfg(target_arch = "wasm32")]
pub use turbosql_impl::Turbosql;
