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

    // --- Proper develop: linear camera RGB -> WB -> color matrix -> gamma ---
    use rawler::imgop::develop::ProcessingStep;
    let cam_dev = RawDevelop {
        steps: vec![
            ProcessingStep::Rescale,
            ProcessingStep::Demosaic,
            ProcessingStep::CropActiveArea,
            ProcessingStep::CropDefault,
        ],
    };
    let cam_inter = cam_dev.develop_intermediate(&raw).expect("camera develop failed");
    let cam_img = cam_inter.to_dynamic_image().expect("camera img").to_rgb16();
    let (cw, ch) = (cam_img.width(), cam_img.height());
    println!(
        "DEBUG cam center px = {:?}  vs default center px = {:?}  (cam dims {cw}x{ch} vs {}x{})",
        cam_img.get_pixel(cw / 2, ch / 2),
        rgb.get_pixel(rgb.width() / 2, rgb.height() / 2),
        rgb.width(),
        rgb.height()
    );

    let xyz2cam = mat_from_slice(
        raw.color_matrix
            .iter()
            .find(|(i, _)| format!("{i:?}") == "D65")
            .or_else(|| raw.color_matrix.iter().next())
            .map(|(_, m)| m.clone())
            .expect("no color matrix"),
    );
    const SRGB_TO_XYZ: [[f32; 3]; 3] = [
        [0.4124564, 0.3575761, 0.1804375],
        [0.2126729, 0.7151522, 0.0721750],
        [0.0193339, 0.1191920, 0.9503041],
    ];
    let cam2rgb = mat_inv(normalize_rows(mat_mul(xyz2cam, SRGB_TO_XYZ)));
    let wb = raw.wb_coeffs;
    let wbn = [wb[0] / wb[1], 1.0, wb[2] / wb[1]];
    println!("cam2rgb = {cam2rgb:?}");

    let l2s = |x: f32| if x <= 0.0031308 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    let mut out8 = image::RgbImage::new(cw, ch);
    let mut sums2 = [0u64; 3];
    for (op, ip) in out8.pixels_mut().zip(cam_img.pixels()) {
        // linear camera RGB, white-balanced
        let cam = [
            ip[0] as f32 / 65535.0 * wbn[0],
            ip[1] as f32 / 65535.0 * wbn[1],
            ip[2] as f32 / 65535.0 * wbn[2],
        ];
        for i in 0..3 {
            let v = (cam2rgb[i][0] * cam[0] + cam2rgb[i][1] * cam[1] + cam2rgb[i][2] * cam[2])
                .clamp(0.0, 1.0);
            op[i] = (l2s(v) * 255.0).round().clamp(0.0, 255.0) as u8;
            sums2[i] += op[i] as u64;
        }
    }
    println!("DEBUG out center px = {:?}", out8.get_pixel(cw / 2, ch / 2));

    // Are the brightest (near-white) pixels neutral? If yes, rawler's color is
    // fine and the "green" is just dim midtones; if green, color is genuinely off.
    let mut bright = [0u64; 3];
    let mut cnt = 0u64;
    for p in rgb.pixels() {
        let luma = p[0] as u32 + p[1] as u32 + p[2] as u32;
        if luma > 240 {
            bright[0] += p[0] as u64;
            bright[1] += p[1] as u64;
            bright[2] += p[2] as u64;
            cnt += 1;
        }
    }
    if cnt > 0 {
        println!(
            "rawler-default brightest pixels ({cnt}) mean = R{:.0} G{:.0} B{:.0}",
            bright[0] as f64 / cnt as f64,
            bright[1] as f64 / cnt as f64,
            bright[2] as f64 / cnt as f64
        );
    }

    // DEFINITIVE: does rawler's WhiteBalance step actually change anything?
    let with_wb = RawDevelop {
        steps: vec![
            ProcessingStep::Rescale, ProcessingStep::Demosaic, ProcessingStep::CropActiveArea,
            ProcessingStep::WhiteBalance, ProcessingStep::Calibrate, ProcessingStep::CropDefault,
            ProcessingStep::SRgb,
        ],
    };
    let without_wb = RawDevelop {
        steps: vec![
            ProcessingStep::Rescale, ProcessingStep::Demosaic, ProcessingStep::CropActiveArea,
            ProcessingStep::Calibrate, ProcessingStep::CropDefault, ProcessingStep::SRgb,
        ],
    };
    let a = with_wb.develop_intermediate(&raw).unwrap().to_dynamic_image().unwrap().to_rgb8();
    let b = without_wb.develop_intermediate(&raw).unwrap().to_dynamic_image().unwrap().to_rgb8();
    println!(
        "WB-on center  = {:?}   WB-off center = {:?}",
        a.get_pixel(a.width() / 2, a.height() / 2),
        b.get_pixel(b.width() / 2, b.height() / 2)
    );
    let n2 = (cw as u64) * (ch as u64);
    println!(
        "proper develop  R={:.1}  G={:.1}  B={:.1}",
        sums2[0] as f64 / n2 as f64,
        sums2[1] as f64 / n2 as f64,
        sums2[2] as f64 / n2 as f64
    );
    out8.save("Z:/Projects/Claude Code/Pixagram/diag_fix.png").expect("save fix failed");
    println!("saved diag_fix.png");
}

fn mat_from_slice(m: Vec<f32>) -> [[f32; 3]; 3] {
    [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]]
}
fn mat_mul(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut o = [[0.0f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}
fn normalize_rows(mut m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    for r in m.iter_mut() {
        let s = r[0] + r[1] + r[2];
        if s.abs() > 1e-9 {
            for v in r.iter_mut() {
                *v /= s;
            }
        }
    }
    m
}
fn mat_inv(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let inv_det = 1.0 / det;
    let mut o = [[0.0f32; 3]; 3];
    o[0][0] = (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det;
    o[0][1] = (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det;
    o[0][2] = (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det;
    o[1][0] = (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det;
    o[1][1] = (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det;
    o[1][2] = (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det;
    o[2][0] = (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det;
    o[2][1] = (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det;
    o[2][2] = (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det;
    o
}
