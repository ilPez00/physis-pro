//! Headless view of the STREAM tab: render one `WorldObject` to stdout.
//!
//!   cargo run --example world_stream_demo -- [directory]
//!
//! This is the same renderer the TUI's STREAM tab uses. It exists so the object
//! model can be inspected without a terminal — and so the "why did this appear"
//! line has a place to be read in a log.

use physis_pro::world_object::WorldObject;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let obj = WorldObject::from_directory(&dir);
    for line in obj.render() {
        println!("{line}");
    }
}
