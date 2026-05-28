fn main() {
    if let Err(error) = glide::benchmark::run_cli() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
