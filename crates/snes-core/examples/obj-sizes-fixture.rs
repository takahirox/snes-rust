//! Build original MIT OBJ cartridges without downloaded code or an assembler.
#[path = "../tests/common/obj_sizes.rs"]
mod obj_sizes;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: obj-sizes-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    for selector in [6, 7] {
        std::fs::write(
            format!("{directory}/obj-sizes-{selector}.sfc"),
            obj_sizes::guest_rom(selector),
        )
        .unwrap();
    }
}
