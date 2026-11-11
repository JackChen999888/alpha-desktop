fn main() {
    if let Err(e) = daozai::pro::bridge::serve() {
        eprintln!("{e}");
    }
}
