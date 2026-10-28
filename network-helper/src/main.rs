//! Native socket backend for the local browser network helper.
fn main() { if let Err(error) = esp_soc::net::helper::run() { eprintln!("network backend: {error}"); std::process::exit(1); } }
