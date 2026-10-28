//! Native guest output snapshot; byte-emitted original MIT source, no host writes.
#[path = "common/raster.rs"]
mod raster;
use snes_core::Machine;

#[test]
fn cpu_dma_guest_raster_snapshot() {
    let mut m = Machine::new(&raster::guest_rom()).unwrap();
    for _ in 0..3 {
        m.run_frame().unwrap();
    }
    assert_eq!(m.bus.peek(0x30), 0x6b);
    assert_eq!((m.framebuffer_width(), m.framebuffer_height()), (256, 224));
    let expected = [
        (0, [0, 40, 51, 77, 102, 138, 149, 167]),
        (1, [0, 47, 59, 84, 110, 145, 157, 174]),
        (100, [0, 42, 53, 79, 104, 140, 151, 169]),
        (223, [0, 46, 58, 83, 109, 144, 156, 173]),
    ];
    let colors = [
        [255, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 119, 0, 255],
        [0, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 255, 255],
    ];
    for (y, edges) in expected {
        let mut spans = vec![];
        let row = &m.framebuffer()[y * 1024..(y + 1) * 1024];
        let mut last = [0; 4];
        for (x, pixel) in row.chunks_exact(4).enumerate() {
            if pixel != last {
                spans.push((x, pixel.to_vec()));
                last.copy_from_slice(pixel);
            }
        }
        println!("row {y}: {spans:?}");
        assert_eq!(
            spans,
            edges
                .into_iter()
                .zip(colors.map(Vec::from))
                .collect::<Vec<_>>()
        );
        // Every pixel including both neighbors of every transition is checked.
        for x in 0..256 {
            let segment = edges.iter().rposition(|edge| *edge <= x).unwrap();
            assert_eq!(&row[x * 4..x * 4 + 4], colors[segment], "({x},{y})");
        }
    }
    let hash = m.framebuffer().iter().fold(0x811c9dc5u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(0x1000193)
    });
    assert_eq!(hash, 0x6041bd2d, "frame3 FNV-1a32 snapshot");
}
