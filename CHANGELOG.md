# Changelog

All changes that are notable to _users_ of this crate will be documented in this file. This specifically excludes minor internal changes (such as changes in test coverage, small documentation fixes, etc.), though major internal changes will be noted. Refer to the commit history if you need more detail.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased blocked pending rusqlite 0.25]

- Add support for struct fields of additional type: `f32`
  - e.g. `struct S { field_f32: Option<f32> }`

## [Unreleased]

### Added

- Added support for `select!` directly into additional type: `f64`
  - e.g. `select!(f64 "bayesian_probability FROM table")`

### Changed

### Deprecated

### Removed

### Fixed

- `.insert()` now returns correct rowid instead of number of affected rows.

### Security

## [0.0.3] - 2021-01-14

### Added

- Added support for `select!` directly into additional types: `String`, `i8`, `u8`, `i16`, `u16`, `i32`, `u32`
  - e.g. `select!(String "post_title FROM table")`

## [0.0.2] - 2021-01-12

- Initial release! 🎉
