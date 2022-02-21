fn main() {
 let mut path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
 while path.file_name() != Some(std::ffi::OsStr::new("target")) {
  path.pop();
 }
 path.pop();
 path.push("migrations.toml");

 println!("cargo:rerun-if-changed={}", path.to_str().unwrap());

 let mut path2 = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
 path2.push("migrations.toml");

 // docs.rs is a largely read-only filesystem
 if std::env::var("DOCS_RS").is_ok() {
  std::fs::write(&path2, "").unwrap();
  return;
 }

 if !path.exists() {
  std::fs::write(&path, "").unwrap();
 }

 std::fs::copy(path, path2).unwrap();
}
