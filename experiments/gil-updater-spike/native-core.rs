// Synthetic Core only: no filesystem, registry, network, or GIL code.
use std::io::{self, BufRead, Write};
fn main() {
    let version = env!("GIL_SPIKE_VERSION");
    if std::env::args().any(|arg| arg == "--probe") {
        if option_env!("GIL_SPIKE_BROKEN") == Some("1") { std::process::exit(7); }
        println!("{{\"version\":\"{}\"}}", version);
        return;
    }
    println!("{}:ready", version);
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        println!("{}:{}", version, line.unwrap());
        io::stdout().flush().unwrap();
    }
}
