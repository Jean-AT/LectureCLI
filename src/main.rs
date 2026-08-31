fn main() {
    if let Err(err) = lecture::run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}
