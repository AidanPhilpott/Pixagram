//! Phase 1 export STUB.
//!
//! The brief asks only to "stub/log an export step to confirm the pipeline
//! reaches end-to-end." We deliberately do not write a file yet. What we *do*
//! prove here is the important architectural claim: the same `Adjustments`
//! definition that drives the GPU preview also arrives in Rust and is applied by
//! the same shared math (`adjustments::apply_pixel`). Real JPEG/border/EXIF
//! export is Phase 5.

use anyhow::Result;

use crate::adjustments::{apply_pixel, Adjustments};

/// Simulate an export and return a human-readable summary for the UI.
pub fn export_stub(path: &str, adj: &Adjustments) -> Result<String> {
    // Demonstrate the shared definition is live on the Rust side by running a
    // sample pixel through the exact same math the shader uses for the preview.
    let sample_in = [0.5_f32, 0.5, 0.5];
    let sample_out = apply_pixel(adj, sample_in);

    log::info!(
        "[export stub] path={path} exposure={:+.2} EV | mid-grey {:.3} -> {:.3} (shared math)",
        adj.exposure,
        sample_in[0],
        sample_out[0],
    );

    Ok(format!(
        "Export stub OK — pipeline reached end-to-end.\n\
         • Source: {path}\n\
         • Exposure: {:+.2} EV\n\
         • Shared math check: mid-grey 0.500 → {:.3}\n\
         (Real JPEG + border + sRGB + EXIF export lands in Phase 5.)",
        adj.exposure, sample_out[0],
    ))
}
