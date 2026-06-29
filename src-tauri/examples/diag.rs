//! Diagnostic: decode + develop one RAF and report color stats.
//! Run: cargo run --example diag -- "C:\\path\\to\\file.RAF"
//!
//! Goal this round: settle whether the residual green/cyan cast survives an
//! owned WB->matrix->sRGB pipeline, and whether the matrix illuminant matters.
//! For each variant we report overall mean balance AND *true* near-white balance
//! (pixels where the DIMMEST channel is already bright — a real white test, not
//! the old luma>240 midtone threshold), then save a small viewable PNG.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

use image::{DynamicImage, RgbImage};
use rawler::decode_file;
use rawler::formats::tiff::ifd::OffsetMode;
use rawler::formats::tiff::IFD;
use rawler::imgop::develop::{ProcessingStep, RawDevelop};
use rawler::rawimage::{RawImage, RawImageData};
use rawler::tags::ExifTag;

const SRGB_TO_XYZ: [[f32; 3]; 3] = [
    [0.4124564, 0.3575761, 0.1804375],
    [0.2126729, 0.7151522, 0.0721750],
    [0.0193339, 0.1191920, 0.9503041],
];

fn main() {
    let path = std::env::args().nth(1).expect("usage: diag <RAF path>");
    let stem = std::path::Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "diag".into());
    dump_wb_tags(&path);

    let raw = decode_file(&path).expect("decode_file failed");

    println!("== raw metadata ==");
    println!("wb_coeffs  = {:?}", raw.wb_coeffs);
    println!("blacklevel = {:?}", raw.blacklevel);
    println!("whitelevel = {:?}", raw.whitelevel);
    println!("make/model = {:?} / {:?}", raw.camera.make, raw.camera.model);
    println!("color_matrix count = {}", raw.color_matrix.len());
    for (illum, m) in raw.color_matrix.iter() {
        println!("  matrix [{illum:?}] = {m:?}");
    }

    // --- 1) rawler's unmodified default develop (what the app currently ships) ---
    raw_mosaic_means(&raw);

    let dev = RawDevelop::default();
    let def_img = dev
        .develop_intermediate(&raw)
        .expect("default develop failed")
        .to_dynamic_image()
        .expect("to_dynamic_image None")
        .to_rgb8();
    report("rawler-default", &def_img);
    save_small(&def_img, &format!("diag_{stem}_default.png"));

    // --- 2) owned pipeline: linear camera RGB -> WB -> cam2rgb -> sRGB ---
    // Pull pre-matrix linear camera RGB (no WhiteBalance / Calibrate / SRgb).
    let cam_dev = RawDevelop {
        steps: vec![
            ProcessingStep::Rescale,
            ProcessingStep::Demosaic,
            ProcessingStep::CropActiveArea,
            ProcessingStep::CropDefault,
        ],
    };
    let cam_img = cam_dev
        .develop_intermediate(&raw)
        .expect("camera develop failed")
        .to_dynamic_image()
        .expect("camera img None")
        .to_rgb16();
    let (cw, ch) = (cam_img.width(), cam_img.height());

    let wb = raw.wb_coeffs;
    let wbn = [wb[0] / wb[1], 1.0, wb[2] / wb[1]];
    println!("\nwb normalized (G=1) = {wbn:?}");

    // --- Stage diagnostics: where does the green come from? ---
    // Pre-WB linear camera means, and the same after applying as-shot WB.
    let np = (cw as u64) * (ch as u64);
    let mut cam_sum = [0f64; 3];
    for ip in cam_img.pixels() {
        cam_sum[0] += ip[0] as f64;
        cam_sum[1] += ip[1] as f64;
        cam_sum[2] += ip[2] as f64;
    }
    let cam_mean = [
        cam_sum[0] / np as f64,
        cam_sum[1] / np as f64,
        cam_sum[2] / np as f64,
    ];
    println!(
        "cam (pre-WB)  mean R={:.0} G={:.0} B={:.0}   R/G={:.3} B/G={:.3}",
        cam_mean[0], cam_mean[1], cam_mean[2],
        cam_mean[0] / cam_mean[1], cam_mean[2] / cam_mean[1]
    );
    println!(
        "cam (post-WB) mean R={:.0} G={:.0} B={:.0}   R/G={:.3} B/G={:.3}",
        cam_mean[0] * wbn[0] as f64, cam_mean[1], cam_mean[2] * wbn[2] as f64,
        cam_mean[0] * wbn[0] as f64 / cam_mean[1], cam_mean[2] * wbn[2] as f64 / cam_mean[1]
    );

    // White-patch auto-WB: multipliers that would neutralize the brightest cam
    // pixels (top 1% by green). If these differ a lot from as-shot WB, the
    // file's WB is the culprit.
    let mut greens: Vec<u16> = cam_img.pixels().map(|p| p[1]).collect();
    greens.sort_unstable();
    let g_thresh = greens[(greens.len() as f64 * 0.99) as usize];
    let mut bs = [0f64; 3];
    let mut bc = 0u64;
    for ip in cam_img.pixels() {
        if ip[1] >= g_thresh {
            bs[0] += ip[0] as f64;
            bs[1] += ip[1] as f64;
            bs[2] += ip[2] as f64;
            bc += 1;
        }
    }
    let bp = [bs[0] / bc as f64, bs[1] / bc as f64, bs[2] / bc as f64];
    println!(
        "white-patch cam (top1% green, n={bc}) R={:.0} G={:.0} B={:.0}  => neutralizing WB R={:.3} G=1 B={:.3}",
        bp[0], bp[1], bp[2], bp[1] / bp[0], bp[1] / bp[2]
    );

    // Run the owned pipeline once per available illuminant matrix so we can see
    // whether matrix choice (A vs D65) is what shifts the white balance.
    let matrices: Vec<(String, Vec<f32>)> = raw
        .color_matrix
        .iter()
        .map(|(i, m)| (format!("{i:?}"), m.clone()))
        .collect();

    // Grey-world auto-WB computed from this image's own camera means. This is the
    // multiplier that makes the average neutral; compare it to rawler's as-shot WB.
    let gw = [
        (cam_mean[1] / cam_mean[0]) as f32,
        1.0f32,
        (cam_mean[1] / cam_mean[2]) as f32,
    ];
    println!(
        "grey-world WB = R={:.3} G=1 B={:.3}   (as-shot R={:.3} B={:.3}; correction x{:.2}/{:.2})",
        gw[0], gw[2], wbn[0], wbn[2], gw[0] / wbn[0], gw[2] / wbn[2]
    );

    // Render owned pipelines: as-shot WB and grey-world WB, both with each matrix.
    for (illum, m) in &matrices {
        let xyz2cam = mat_from_slice(m.clone());
        let cam2rgb = mat_inv(normalize_rows(mat_mul(xyz2cam, SRGB_TO_XYZ)));

        for (wbtag, wbm) in [("asshot", wbn), ("greyworld", gw)] {
            // Apply a fixed exposure lift so brightness is comparable to the OOC
            // JPEG (Fuji applies a tone/brightness curve; linear develop is dark).
            // Color-neutral gain — purely to judge WB, not the final tone curve.
            const LIFT: f32 = 4.0;
            let mut out = RgbImage::new(cw, ch);
            for (op, ip) in out.pixels_mut().zip(cam_img.pixels()) {
                let cam = [
                    ip[0] as f32 / 65535.0 * wbm[0] * LIFT,
                    ip[1] as f32 / 65535.0 * wbm[1] * LIFT,
                    ip[2] as f32 / 65535.0 * wbm[2] * LIFT,
                ];
                for i in 0..3 {
                    let v = (cam2rgb[i][0] * cam[0]
                        + cam2rgb[i][1] * cam[1]
                        + cam2rgb[i][2] * cam[2])
                        .clamp(0.0, 1.0);
                    op[i] = (linear_to_srgb(v) * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
            let tag = format!("owned[{illum}/{wbtag}]");
            report(&tag, &out);
            save_small(&out, &format!("diag_{stem}_owned_{illum}_{wbtag}.png"));
        }
    }
}

/// Report overall mean balance and *true* near-white balance.
fn report(tag: &str, img: &RgbImage) {
    let (w, h) = (img.width(), img.height());
    let n = (w as u64) * (h as u64);
    let mut sum = [0u64; 3];
    for p in img.pixels() {
        sum[0] += p[0] as u64;
        sum[1] += p[1] as u64;
        sum[2] += p[2] as u64;
    }
    println!(
        "\n[{tag}] {w}x{h}  mean R={:.1} G={:.1} B={:.1}",
        sum[0] as f64 / n as f64,
        sum[1] as f64 / n as f64,
        sum[2] as f64 / n as f64
    );
    // Near-white = the DIMMEST channel is already bright. This isolates genuine
    // whites (a neutral white has R~=G~=B all high), not single-channel highlights
    // and not the old confounded luma>240 midtone bucket.
    for thresh in [200u8, 230, 245] {
        let mut s = [0u64; 3];
        let mut c = 0u64;
        for p in img.pixels() {
            if p[0].min(p[1]).min(p[2]) >= thresh {
                s[0] += p[0] as u64;
                s[1] += p[1] as u64;
                s[2] += p[2] as u64;
                c += 1;
            }
        }
        if c > 0 {
            let (r, g, b) = (
                s[0] as f64 / c as f64,
                s[1] as f64 / c as f64,
                s[2] as f64 / c as f64,
            );
            // Cast indicator relative to green (the reference channel).
            println!(
                "    near-white(min>={thresh}) n={c:>8}  R={r:.0} G={g:.0} B={b:.0}   R/G={:.3} B/G={:.3}",
                r / g,
                b / g
            );
        } else {
            println!("    near-white(min>={thresh}) none");
        }
    }
}

/// Save a downscaled (longest edge ~1000px) copy for quick visual inspection.
fn save_small(img: &RgbImage, name: &str) {
    let dyn_img = DynamicImage::ImageRgb8(img.clone());
    let small = dyn_img.resize(1000, 1000, image::imageops::FilterType::Lanczos3);
    let out = format!("Z:/Projects/Claude Code/Pixagram/{name}");
    small.save(&out).expect("save failed");
    println!("    saved {out}");
}

/// THE definitive test: average the raw sensor values per CFA color BEFORE any
/// demosaic, black-subtracted. If the camera's as-shot WB neutralizes these,
/// then the metadata is self-consistent and any cast is introduced by rawler's
/// develop (demosaic/scaling). If it does NOT neutralize, the black level / WB
/// interpretation itself is the problem.
fn raw_mosaic_means(raw: &RawImage) {
    let data = match &raw.data {
        RawImageData::Integer(d) => d,
        RawImageData::Float(_) => {
            println!("== raw mosaic: float data, skipping ==");
            return;
        }
    };
    let (w, h) = (raw.width, raw.height);
    let cfa = &raw.camera.cfa;
    let black = raw
        .blacklevel
        .levels
        .first()
        .map(|r| r.as_f32())
        .unwrap_or(0.0) as f64;
    let white = raw.whitelevel.as_vec().first().copied().unwrap_or(65535.0) as f64;

    let mut sum = [0f64; 3];
    let mut cnt = [0u64; 3];
    for row in 0..h {
        for col in 0..w {
            let c = cfa.color_at(row, col);
            if c < 3 {
                let v = (data[row * w + col] as f64 - black).max(0.0);
                sum[c] += v;
                cnt[c] += 1;
            }
        }
    }
    let m = [
        sum[0] / cnt[0].max(1) as f64,
        sum[1] / cnt[1].max(1) as f64,
        sum[2] / cnt[2].max(1) as f64,
    ];
    println!("== RAW MOSAIC (pre-demosaic, black={black}, white={white}) ==");
    println!(
        "  site counts R={} G={} B={}  (X-Trans ~ 8:20:8 per 6x6 tile)",
        cnt[0], cnt[1], cnt[2]
    );
    println!(
        "  per-channel mean R={:.1} G={:.1} B={:.1}   R/G={:.3} B/G={:.3}",
        m[0], m[1], m[2], m[0] / m[1], m[2] / m[1]
    );
    let wb = [
        raw.wb_coeffs[0] / raw.wb_coeffs[1],
        1.0,
        raw.wb_coeffs[2] / raw.wb_coeffs[1],
    ];
    println!(
        "  after as-shot WB [{:.3},1,{:.3}]: R/G={:.3} B/G={:.3}  (==1.0 means metadata neutralizes the sensor)",
        wb[0], wb[2],
        m[0] * wb[0] as f64 / m[1],
        m[2] * wb[2] as f64 / m[1]
    );
}

/// Replicate just enough of rawler's RAF parse to read the Fuji makernote WB
/// tags directly, so we can see which one holds the *as-shot* WB (the one that
/// neutralizes the scene), since rawler's RawImage.wb_coeffs is the wrong set.
fn dump_wb_tags(path: &str) {
    const RAF_TIFF1_PTR_OFFSET: u64 = 84;
    const RAF_TIFF2_PTR_OFFSET: u64 = 100;
    let mut stream = BufReader::new(File::open(path).expect("open RAF"));
    let read_be = |s: &mut BufReader<File>, at: u64| -> u32 {
        s.seek(SeekFrom::Start(at)).unwrap();
        let mut b = [0u8; 4];
        s.read_exact(&mut b).unwrap();
        u32::from_be_bytes(b)
    };
    let offset = read_be(&mut stream, RAF_TIFF1_PTR_OFFSET);
    let main = IFD::new_root(&mut stream, offset + 12).expect("parse main IFD");

    // Second TIFF = the FujiIFD (0xf000) block, holding the WB level tags.
    let ioffset = read_be(&mut stream, RAF_TIFF2_PTR_OFFSET);
    let fuji = IFD::new_root_with_correction(&mut stream, 0, ioffset, 0, 10, &[0xf000u16]).ok();

    let mn = main
        .find_first_ifd_with_tag(ExifTag::MakerNotes)
        .and_then(|ifd| {
            ifd.parse_makernote(&mut stream, OffsetMode::Absolute, &[])
                .ok()
                .flatten()
        });

    println!("== Fuji WB tags (R/G, B/G after green-normalize) ==");
    // 0xf00c Standard, 0xf00d Auto, 0xf00e GRBLevels live in the FujiIFD block;
    // 0x2ff0 GRGBLevels in the makernotes. GRB = [G,R,B]; GRGB = [G,R,G,B].
    for (name, tag, order) in [
        ("WB_GRBLevelsStandard", 0xf00cu16, "GRB"),
        ("WB_GRBLevelsAuto", 0xf00du16, "GRB"),
        ("WB_GRBLevels", 0xf00eu16, "GRB"),
        ("WB_GRGBLevels", 0x2ff0u16, "GRGB"),
    ] {
        let found = fuji
            .as_ref()
            .and_then(|f| f.find_first_ifd_with_tag(tag).and_then(|i| i.get_entry(tag)))
            .or_else(|| mn.as_ref().and_then(|m| m.get_entry(tag)))
            .or_else(|| main.get_entry(tag));
        match found {
            Some(e) => {
                let n = e.count() as usize;
                let v: Vec<f32> = (0..n).map(|i| e.value.force_f32(i)).collect();
                let (r, g, b) = match order {
                    "GRGB" if n >= 4 => (v[1], (v[0] + v[2]) * 0.5, v[3]),
                    _ if n >= 3 => (v[1], v[0], v[2]),
                    _ => (f32::NAN, f32::NAN, f32::NAN),
                };
                println!(
                    "  {name:<22} {tag:#06x} raw={v:?}  =>  R/G={:.3} B/G={:.3}",
                    r / g,
                    b / g
                );
            }
            None => println!("  {name:<22} {tag:#06x} (absent)"),
        }
    }
}

#[inline]
fn linear_to_srgb(x: f32) -> f32 {
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

fn mat_from_slice(m: Vec<f32>) -> [[f32; 3]; 3] {
    [
        [m[0], m[1], m[2]],
        [m[3], m[4], m[5]],
        [m[6], m[7], m[8]],
    ]
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
