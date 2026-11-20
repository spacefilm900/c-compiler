fn main() {
    let args: Vec<String> = std::env::args().collect();
    std::process::exit(hcc::run(&args));
}
