//! Original MIT natural-scheduler regression for Issue #65.
use snes_core::Machine;
#[path = "common/mode7_row.rs"]
mod mode7_row;

fn check_guest(flipped: bool, bg2: bool) {
    let mut m = Machine::new(&mode7_row::guest_rom(flipped, bg2)).unwrap();
    while m.bus.peek(0x30) != 0x65 && m.cpu.instructions < 300_000 {
        m.step();
        assert!(!m.cpu.stopped);
        assert!(m.bus.apu.error.is_none());
        assert!(m.bus.frames < 8, "startup frame deadline");
    }
    assert_eq!(m.bus.peek(0x30), 0x65, "startup instruction deadline");
    m.run_frame().unwrap(); // discard the frame containing setup/forced blank
    for _ in 0..3 {
        let instructions = m.cpu.instructions;
        m.run_frame().unwrap(); // rejects CPU/APU errors; bounded to 400000 clocks
        assert!(m.cpu.instructions - instructions < 100_000);
        assert_eq!(
            (
                m.framebuffer_width(),
                m.framebuffer_height(),
                m.framebuffer().len()
            ),
            (256, 224, 229376)
        );
        for (y, color) in [
            (0, [255, 0, 0, 255]),
            (9, [0, 0, 255, 255]),
            (223, [255, 255, 0, 255]),
        ] {
            for (x, expected) in [(6, [0, 255, 0, 255]), (7, color), (8, [0, 255, 0, 255])] {
                let a = (y * 256 + x) * 4;
                assert_eq!(
                    &m.framebuffer()[a..a + 4],
                    expected,
                    "flip={flipped},bg2={bg2},({x},{y})"
                );
            }
        }
        // Adjacent rows do not repeat the marker artwork.
        for y in [1, 8, 10, 222] {
            let a = (y * 256 + 7) * 4;
            assert_eq!(&m.framebuffer()[a..a + 4], [0, 255, 0, 255]);
        }
        assert_eq!(m.bus.peek(0x30), 0x65);
    }
}
#[test]
fn bg1_normal() {
    check_guest(false, false);
}
#[test]
fn bg2_normal() {
    check_guest(false, true);
}
#[test]
fn bg1_flipped() {
    check_guest(true, false);
}
#[test]
fn bg2_flipped() {
    check_guest(true, true);
}
