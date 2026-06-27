//! Diagnostic: decode + develop one RAF and report color stats.
//! Run: cargo run --example diag -- "C:\\path\\to\\file.RAF"

use rawler::decode_file;
use rawler::imgop::develop::RawDevelop;

fn main() {
    let path = std::env::args().nth(1).expect("usage: diag <RAF path>");
    let raw = decode_file(&path).expect("decode_file failed");

    println!("== raw metadata ==");
    println!("wb_coeffs  = {:?}", raw.wb_coeffs);
    println!("cfa        = {:?}", raw.camera.cfa);
    println!("cpp        = {}", raw.cpp);
    println!("dim        = {:?}", raw.dim());
    println!("blacklevel = {:?}", raw.blacklevel);
    println!("whitelevel = {:?}", raw.whitelevel);
    println!("make/model = {:?} / {:?}", raw.camera.make, raw.camera.model);
    println!("color_matrix count = {}", raw.color_matrix.len());
    for (illum, m) in raw.color_matrix.iter() {
        println!("  matrix [{:?}] = {:?}", illum, m);
    }

    let dev = RawDevelop::default();
    let inter = dev.develop_intermediate(&raw).expect("develop failed");
    let img = inter.to_dynamic_image().expect("to_dynamic_image returned None");
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width(), rgb.height());

    let mut sums = [0u64; 3];
    for p in rgb.pixels() {
        sums[0] += p[0] as u64;
        sums[1] += p[1] as u64;
        sums[2] += p[2] as u64;
    }
    let n = (w as u64) * (h as u64);
    println!("== developed {w}x{h} ==");
    println!(
        "mean  R={:.1}  G={:.1}  B={:.1}   (balanced grey-world ≈ equal)",
        sums[0] as f64 / n as f64,
        sums[1] as f64 / n as f64,
        sums[2] as f64 / n as f64
    );

    let out = "Z:/Projects/Claude Code/Pixagram/diag_out.png";
    rgb.save(out).expect("save failed");
    println!("saved {out}");

    // Experiment: apply as-shot WB as a direct gain on the encoded sRGB values
    // (matching raf::apply_as_shot_wb), normalized so green = 1.0.
    let wb = raw.wb_coeffs;
    let gain = [wb[0] / wb[1], 1.0, wb[2] / wb[1]];
    let mut wbimg = rgb.clone();
    let mut sums2 = [0u64; 3];
    for p in wbimg.pixels_mut() {
        for i in 0..3 {
            p[i] = (p[i] as f32 * gain[i]).min(255.0) as u8;
            sums2[i] += p[i] as u64;
        }
    }
    println!(
        "after *wb  R={:.1}  G={:.1}  B={:.1}",
        sums2[0] as f64 / n as f64,
        sums2[1] as f64 / n as f64,
        sums2[2] as f64 / n as f64
    );
    wbimg
        .save("Z:/Projects/Claude Code/Pixagram/diag_wb.png")
        .expect("save wb failed");
    println!("saved diag_wb.png");
}
