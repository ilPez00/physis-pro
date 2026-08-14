//! physis-pro-web — thin entry point.
//!
//! The server itself lives in `physis_pro::web` so the router can be built and
//! exercised from `tests/` without binding a port.

#[tokio::main]
async fn main() {
    physis_pro::web::serve().await;
}
