fn main() {
 let mut path = std::path::PathBuf::new();
 path.push(std::env::var_os("OUT_DIR").unwrap());
 while path.file_name() != Some(std::ffi::OsStr::new("target")) {
  path.pop();
 }
 path.pop();
 path.push("migrations.toml");

 if !path.exists() {
  std::fs::write(&path, "").unwrap();
 }

 let mut path2 = std::path::PathBuf::new();
 path2.push(std::env::var_os("OUT_DIR").unwrap());
 path2.push("migrations.toml");

 std::fs::hard_link(path, path2).ok();
}
