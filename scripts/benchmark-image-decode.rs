//! Standalone probe for the production bounded image decoder. Run one format
//! per process so each peak excludes allocations left by previous decodes.
//! Run with `cargo run -p flectar-renderer-probe --bin benchmark-image-decode
//! --release -- png|jpeg|webp|gif [target_dimension]`.
//! Requested-live heap bytes exclude allocator overhead, DOM, paint cache,
//! renderer tiles, and process RSS/PSS.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    io::Cursor,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use blitz_dom::net::ImageDecodeLimits;
use image::{ImageFormat, Rgb, RgbImage};

struct CountingAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            let live = if new_size >= layout.size() {
                LIVE.fetch_add(new_size - layout.size(), Ordering::Relaxed) + new_size
                    - layout.size()
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed)
                    - (layout.size() - new_size)
            };
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        replacement
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

fn main() {
    let format = match std::env::args().nth(1).as_deref() {
        Some("png") => ImageFormat::Png,
        Some("jpeg") => ImageFormat::Jpeg,
        Some("webp") => ImageFormat::WebP,
        Some("gif") => ImageFormat::Gif,
        _ => panic!("expected png, jpeg, webp, or gif"),
    };
    let target = std::env::args()
        .nth(2)
        .map(|value| value.parse::<u32>().expect("target dimension"))
        .unwrap_or(2048);
    let width = 3000;
    let height = 2000;
    let source = RgbImage::from_fn(width, height, |x, y| {
        Rgb([(x / 12) as u8, (y / 8) as u8, ((x + y) / 20) as u8])
    });
    let mut encoded = Cursor::new(Vec::new());
    source
        .write_to(&mut encoded, format)
        .expect("encode fixture");
    drop(source);
    let bytes = encoded.into_inner();
    let limits = ImageDecodeLimits {
        max_source_dimension: 4096,
        max_source_pixels: 8 * 1024 * 1024,
        max_jpeg_source_dimension: 16384,
        max_jpeg_source_pixels: 64 * 1024 * 1024,
        max_alloc: 8 * 1024 * 1024 * 4,
        target_dimension: target,
    };
    let baseline = LIVE.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);
    let started = Instant::now();
    let decoded = limits.decode(&bytes).expect("decode fixture");
    let elapsed = started.elapsed();
    let peak = PEAK.load(Ordering::Relaxed) - baseline;
    let settled = LIVE.load(Ordering::Relaxed) - baseline;
    assert_eq!((decoded.width, decoded.height), (width, height));
    assert_eq!(
        decoded.data.len(),
        (decoded.pixel_width * decoded.pixel_height * 4) as usize
    );
    println!(
        "{format:?} target={target} source={}x{} retained={}x{} encoded={} peak_extra={} settled_extra={} elapsed_ms={:.2}",
        width,
        height,
        decoded.pixel_width,
        decoded.pixel_height,
        bytes.len(),
        peak,
        settled,
        elapsed.as_secs_f64() * 1000.0,
    );
}
