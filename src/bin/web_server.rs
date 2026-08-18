//! physis-pro-web — thin entry point.
//!
//! The server itself lives in `physis_pro::web` so the router can be built and
//! exercised from `tests/` without binding a port.

#[tokio::main]
async fn main() {
    // Gate before binding a port. A server that starts and then serves 402s
    // still exposes the surface to scanners; refusing to boot is cleaner and
    // gives the operator one unambiguous line in the logs.
    let license = physis_pro::license::enforce();
    if let Err(e) = license.require("web") {
        eprintln!("physis: {e}");
        std::process::exit(2);
    }
    physis_pro::web::serve().await;
}
