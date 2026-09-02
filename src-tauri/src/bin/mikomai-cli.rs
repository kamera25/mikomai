fn main() {
    if let Err(error) = mikomai_lib::cli::run() {
        eprintln!("mikomai: {error}");
        std::process::exit(1);
    }
}
