//! Font loading + measurement utilities for export rendering.
//!
//! This module is the foundation for replacing the heuristic `text_metrics`
//! function inside `export.rs` with real (or closer to real) font metrics.
//!
//! Strategy
//! --------
//! 1. When the Cargo feature `embed_inter` is enabled we embed the Inter
//!    font weights we actively use (Regular, SemiBold, Bold) and construct
//!    `fontdue::Font` instances for measurement.
//! 2. When the feature is NOT enabled (or the embed fails for any reason),
//!    we fall back to heuristic metrics that reproduce the current layout
//!    so the export pipeline does not break for contributors who have not
//!    yet added the font files locally.
//!
//! Why the feature gate?
//! ---------------------
//! - The repository currently does not include the Inter TTFs. Adding them
//!   will increase repo size slightly and they are licensed under the SIL
//!   Open Font License (compatible). Until they are committed, the fallback
//!   avoids compilation failures.
//! - Once the fonts are added to `ui/assets/`, you can enable the feature
//!   (e.g. `--features embed_inter`) to switch the export code to real
//!   metrics, then remove the fallback once the transition is complete.
//!
//! Expected variable font file locations (relative to this file):
//! - ../../assets/Inter-Variable.ttf
//! - ../../assets/Inter-Italic-Variable.ttf (optional)
//!
//! Add a short NOTICE or LICENSE snippet for Inter under something like
//! `ui/assets/licenses/INTER.txt` when you commit the font binaries.
//!
//! Integration steps in `export.rs` (future patch):
//! -----------------------------------------------
//! - Instantiate `let fonts = fonts::Fonts::load();`
//! - Replace calls to the old `text_metrics(size)` with either
//!   `fonts.metrics(FontWeight::Bold, 56.0)` etc.
//! - Use `tm.asc` as the baseline; advance subsequent baselines by
//!   `tm.line_h + spacing` just like before.
//!
//! NOTE on accuracy
//! ----------------
//! `fontdue` provides glyph metrics; it does not expose full typographic
//! ascent/descent tables directly. We approximate:
//! - line height: max(reported glyph height, size * 1.22) to avoid tight
//!   caps-only measurements collapsing vertical rhythm.
//! - ascender:  size * ASCENDER_RATIO (empirically tuned for Inter).
//! - descender: line_height - ascender
//!
//! This gives us a deterministic improvement over the previous magic
//! constants while remaining stable if/when the underlying glyph bounds
//! differ between platforms.
//!
//! Once you embed the fonts you may refine the ratios or swap to using a
//! composite sample set (e.g. "Hg") to better estimate vertical metrics.

#![allow(dead_code)]

use std::fmt;

#[cfg(feature = "embed_inter")]
use fontdue::Font;

/// Lightweight weight indicator so callers avoid stringly-typed lookups.
#[derive(Clone, Copy, Debug)]
pub enum FontWeight {
    Regular,
    SemiBold,
    Bold,
}

impl fmt::Display for FontWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FontWeight::Regular => "Regular",
            FontWeight::SemiBold => "SemiBold",
            FontWeight::Bold => "Bold",
        })
    }
}

/// Public metrics struct used by export layout.
#[derive(Clone, Copy, Debug)]
pub struct TextMetrics {
    /// Chosen vertical line height for layout rhythm.
    pub line_h: f64,
    /// Estimated ascender distance above baseline.
    pub asc: f64,
    /// Estimated descender distance below baseline (positive number).
    pub desc: f64,
}

/// Container for embedded fonts (when feature enabled).
#[cfg(feature = "embed_inter")]
pub struct Fonts {
    regular: Font,
    semibold: Font,
    bold: Font,
}

/// Placeholder container when fonts are not embedded.
#[cfg(not(feature = "embed_inter"))]
pub struct Fonts;

impl Fonts {
    /// Load fonts (embedded or fallback).
    pub fn load() -> Self {
        #[cfg(feature = "embed_inter")]
        {
            // Variable font approach:
            // Inter now distributes a variable font that covers the full upright weight range.
            // We load the upright variable file and (optionally) the italic file. For the
            // purposes of vertical metrics, weight differences do not materially change
            // ascender/descender we care about, so we point all weight requests to the same
            // underlying font object.
            const VAR_BYTES: &[u8] = include_bytes!("../../assets/Inter-Variable.ttf");
            const VAR_ITALIC_BYTES: &[u8] =
                include_bytes!("../../assets/Inter-Italic-Variable.ttf");

            let variable = Font::from_bytes(VAR_BYTES, Default::default())
                .expect("Inter Variable font parse failed");
            // Attempt to parse italic; if missing or invalid, reuse the upright variable.
            let _italic = Font::from_bytes(VAR_ITALIC_BYTES, Default::default())
                .unwrap_or_else(|_| variable.clone());

            let regular = variable.clone();
            let semibold = variable.clone();
            let bold = variable;

            Fonts {
                regular,
                semibold,
                bold,
            }
        }
        #[cfg(not(feature = "embed_inter"))]
        {
            // Fallback: no actual font objects needed.
            Fonts
        }
    }

    /// Obtain text metrics for the given weight + size (px).
    ///
    /// If the real fonts are available we sample a representative glyph
    /// (uppercase 'M') for its reported height, then normalize. Otherwise
    /// we reproduce the previous heuristic so layout diffs remain small.
    pub fn metrics(&self, weight: FontWeight, size_px: f64) -> TextMetrics {
        #[cfg(feature = "embed_inter")]
        {
            use fontdue::Font;

            let font: &Font = match weight {
                FontWeight::Regular => &self.regular,
                FontWeight::SemiBold => &self.semibold,
                FontWeight::Bold => &self.bold,
            };

            // Representative capital letter (broad vertical coverage).
            let m = font.metrics('M', size_px as f32);

            // Heuristic normalization:
            // Inter’s natural preferred line height in UI usage tends to be ~1.25–1.30 of the font size
            // when accounting for leading and balancing asc/desc visually.
            let raw_h = m.height as f64;
            let min_target = size_px * 1.24;
            let line_h = raw_h.max(min_target).ceil();

            // Ascender ratio tuned to keep baseline alignment stable relative to previous 0.92
            // while giving a little more breathing room when real glyph metrics are tighter.
            let asc_ratio = 0.90;
            let asc = (size_px * asc_ratio).round();
            let desc = (line_h - asc).max(size_px * 0.08).round();

            TextMetrics { line_h, asc, desc }
        }

        #[cfg(not(feature = "embed_inter"))]
        {
            // Legacy heuristic (kept so diffs are minimal until feature is enabled).
            let line_h = (size_px * 1.28).round();
            let asc = (size_px * 0.92).round();
            let desc = (line_h - asc).max(size_px * 0.08).round();
            TextMetrics { line_h, asc, desc }
        }
    }
}

/// Convenience helper for callers who don't want to hold a `Fonts` instance.
/// Prefer caching the `Fonts` in the calling scope instead of using this
/// per-measure call in a tight loop.
pub fn measure(weight: FontWeight, size_px: f64) -> TextMetrics {
    // A lightweight static so we only load fonts once.
    static mut FONTS_ONCE: Option<Fonts> = None;
    // SAFETY: write once pattern; export rendering is single-threaded in current design.
    unsafe {
        if FONTS_ONCE.is_none() {
            FONTS_ONCE = Some(Fonts::load());
        }
        FONTS_ONCE
            .as_ref()
            .expect("Fonts not initialized")
            .metrics(weight, size_px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_increase_with_size() {
        let small = measure(FontWeight::Regular, 12.0);
        let large = measure(FontWeight::Regular, 48.0);
        assert!(large.line_h > small.line_h);
        assert!(large.asc > small.asc);
    }

    #[test]
    fn baseline_consistency_ratio() {
        let m = measure(FontWeight::SemiBold, 32.0);
        let baseline_ratio = m.asc / 32.0;
        // Ensure ratio is within an expected envelope (heuristic guard).
        assert!(baseline_ratio > 0.80 && baseline_ratio < 1.05);
    }
}
