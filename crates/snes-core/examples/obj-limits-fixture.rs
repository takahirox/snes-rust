//! Build original MIT OBJ-limit cartridges; no assembler or external assets.
#[path = "../tests/common/obj_limits.rs"]
mod obj_limits;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: obj-limits-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    for mode in 0..4 {
        std::fs::write(
            format!("{directory}/obj-limits-{mode}.sfc"),
            obj_limits::guest_rom(mode),
        )
        .unwrap();
    }
}
