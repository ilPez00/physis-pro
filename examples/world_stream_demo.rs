//! Headless view of the STREAM tab: project a path and walk one step inside it.
//!
//!   cargo run --example world_stream_demo -- [file-or-directory]
//!
//! A directory prints the project object and its immediate children; a file
//! prints the file object and its structural symbols. Same renderer the TUI
//! uses, so the object model can be inspected without a terminal — and the
//! "why did this appear" line has a place to be read in a log.

use physis_pro::world_object::WorldObject;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let obj = WorldObject::from_path(&path);
    for line in obj.render() {
        println!("{line}");
    }
    let children = obj.children();
    println!("\ninside ({}):", children.len());
    for child in children.iter().take(40) {
        println!("  {:?}  {}", child.kind, child.content);
    }
}
