//! Validate the *production* owned develop against the camera JPEG.
//! Run: cargo run --release --example develop_check -- "C:\\path\\to\\file.RAF"
//!
//! Saves a ~1000px PNG of exactly what the app's preview path produces, plus the
//! near-white balance so we can confirm neutrality numerically.

use std::path::Path;

fn main() {
    let path = std::env::args().nth(1).expect("usage: develop_check <RAF path>");
    let stem = Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into());

    // Mirror the preview path's max edge (2048), then save a small copy.
    let img = pixagram_lib::develop_for_test(Path::new(&path), 2048).expect("develop failed");
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());

    let mut sum = [0u64; 3];
    for p in rgb.pixels() {
        sum[0] += p[0] as u64;
        sum[1] += p[1] as u64;
        sum[2] += p[2] as u64;
    }
    let n = (w as u64 * h as u64) as f64;
    println!("{stem}: {w}x{h}  mean R={:.1} G={:.1} B={:.1}", sum[0] as f64 / n, sum[1] as f64 / n, sum[2] as f64 / n);
    for thresh in [180u8, 210, 235] {
        let (mut s, mut c) = ([0u64; 3], 0u64);
        for p in rgb.pixels() {
            if p[0].min(p[1]).min(p[2]) >= thresh {
                s[0] += p[0] as u64; s[1] += p[1] as u64; s[2] += p[2] as u64; c += 1;
            }
        }
        if c > 0 {
            let (r, g, b) = (s[0] as f64 / c as f64, s[1] as f64 / c as f64, s[2] as f64 / c as f64);
            println!("  near-white(min>={thresh}) n={c:>7}  R={r:.0} G={g:.0} B={b:.0}  R/G={:.3} B/G={:.3}", r / g, b / g);
        }
    }

    let small = img.resize(1000, 1000, image::imageops::FilterType::Lanczos3);
    let out = format!("Z:/Projects/Claude Code/Pixagram/check_{stem}.png");
    small.save(&out).expect("save failed");
    println!("  saved {out}");
}
