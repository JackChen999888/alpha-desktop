fn main() {
    if std::env::args().any(|a| a == "--install-hooks") {
        match daozai::pro::agent::install_hooks() {
            Ok(v) => println!("{v}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    // Advisory observer only: never block the originating agent or return decisions.
    let _ = daozai::pro::agent::ingest_cli();
    println!("{{}}");
}
