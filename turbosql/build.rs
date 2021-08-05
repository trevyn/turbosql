fn main() {
 let mut path = std::path::PathBuf::new();
 path.push(std::env::var_os("OUT_DIR").unwrap());
 path.pop();
 path.pop();
 path.pop();
 path.pop();
 path.pop();
 path.push("migrations.toml");

 if !path.exists() {
  std::fs::write(path, "").unwrap();
 }
}
