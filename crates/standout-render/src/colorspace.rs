//! Theme-relative colorspace for perceptually uniform palette generation.
//!
//! # Motivation
//!
//! Terminal 256-color palettes have 240 extended colors (indices 16–255) that are
//! hardcoded with fixed RGB values, completely ignoring the user's base16 theme.
//! This module implements [jake-stewart's proposal][gist] to generate those colors
//! by **trilinear interpolation in CIE LAB space** using the 8 base ANSI colors
//! as cube corners.
//!
//! [gist]: https://gist.github.com/jake-stewart/0a8ea46159a7da2c808e5be2177e1783
//!
//! # Concept: Theme-Relative Color
//!
//! Instead of addressing colors as absolute RGB values, this module lets you specify
//! a **position in a color cube** whose corners are the user's theme colors:
//!
//! | Cube corner | ANSI color |
//! |-------------|------------|
//! | `(0, 0, 0)` | background (defaults to black) |
//! | `(1, 0, 0)` | red |
//! | `(0, 1, 0)` | green |
//! | `(1, 1, 0)` | yellow |
//! | `(0, 0, 1)` | blue |
//! | `(1, 0, 1)` | magenta |
//! | `(0, 1, 1)` | cyan |
//! | `(1, 1, 1)` | foreground (defaults to white) |
//!
//! A [`CubeCoord`] like `(0.6, 0.2, 0.0)` means "60% toward red, 20% toward
//! green, 0% blue" — the theme determines what that actually looks like on screen.
//!
//! # Why CIE LAB?
//!
//! LAB is a **perceptually uniform** colorspace: equal numerical distances correspond
//! to equal perceived color differences. Interpolating in LAB (rather than RGB) ensures:
//!
//! - **Consistent brightness**: blue shades at level 3 look as bright as green at level 3
//! - **Smooth gradients**: no muddy midpoints or perceptual jumps
//! - **Hue preservation**: interpolation follows natural color transitions
//!
//! # Trilinear Interpolation
//!
//! The 8 theme colors sit at the corners of a unit cube. For any point `(r, g, b)`
//! inside the cube, the color is computed by three nested linear interpolations in LAB:
//!
//! 1. **R-axis**: Interpolate 4 edge pairs (bg→red, green→yellow, blue→magenta, cyan→fg)
//! 2. **G-axis**: Interpolate between the R-axis results to sweep across 2 faces
//! 3. **B-axis**: Interpolate between the G-axis results to fill the volume
//!
//! At every grid point, the resulting color is a smooth blend of all 8 corner colors,
//! weighted by proximity.
//!
//! # Example
//!
//! ```rust
//! use standout_render::colorspace::{CubeCoord, Rgb, ThemePalette};
//!
//! // Define a gruvbox-like palette (8 base colors)
//! let palette = ThemePalette::new([
//!     Rgb(40, 40, 40),     // black
//!     Rgb(204, 36, 29),    // red
//!     Rgb(152, 151, 26),   // green
//!     Rgb(215, 153, 33),   // yellow
//!     Rgb(69, 133, 136),   // blue
//!     Rgb(177, 98, 134),   // magenta
//!     Rgb(104, 157, 106),  // cyan
//!     Rgb(168, 153, 132),  // white
//! ]);
//!
//! // Resolve a theme-relative coordinate to an actual RGB color
//! let coord = CubeCoord::from_percentages(60.0, 20.0, 0.0).unwrap();
//! let color = palette.resolve(&coord);
//!
//! // Generate a full 240-color extended palette (216 cube + 24 grayscale)
//! let extended = palette.generate_palette(6);
//! assert_eq!(extended.len(), 240);
//! ```

// ─── RGB type ───────────────────────────────────────────────────────────────

/// A simple RGB color triplet.
///
/// This is the module's own RGB type, decoupled from any terminal or styling crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

// ─── CIE LAB internals ─────────────────────────────────────────────────────

/// CIE LAB color (internal representation for perceptually uniform interpolation).
#[derive(Debug, Clone, Copy)]
struct Lab {
    l: f64,
    a: f64,
    b: f64,
}

/// D65 reference white point for CIE XYZ → LAB conversion.
const XN: f64 = 0.95047;
const YN: f64 = 1.00000;
const ZN: f64 = 1.08883;

/// Convert an sRGB component (0–255) to linear light (0.0–1.0).
fn srgb_to_linear(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert a linear light value (0.0–1.0) to sRGB (0–255), clamped.
fn linear_to_srgb(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

/// LAB forward transform helper.
fn lab_f(t: f64) -> f64 {
    if t > 0.008856 {
        t.cbrt()
    } else {
        7.787 * t + 16.0 / 116.0
    }
}

/// LAB inverse transform helper.
fn lab_f_inv(t: f64) -> f64 {
    if t > 0.206896 {
        t * t * t
    } else {
        (t - 16.0 / 116.0) / 7.787
    }
}

/// Convert an [`Rgb`] value to CIE LAB via XYZ (D65 illuminant).
fn rgb_to_lab(rgb: Rgb) -> Lab {
    let r = srgb_to_linear(rgb.0);
    let g = srgb_to_linear(rgb.1);
    let b = srgb_to_linear(rgb.2);

    // sRGB → XYZ (D65) using the standard matrix
    let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;

    let fx = lab_f(x / XN);
    let fy = lab_f(y / YN);
    let fz = lab_f(z / ZN);

    Lab {
        l: 116.0 * fy - 16.0,
        a: 500.0 * (fx - fy),
        b: 200.0 * (fy - fz),
    }
}

/// Convert a CIE LAB value back to [`Rgb`] via XYZ (D65 illuminant).
fn lab_to_rgb(lab: Lab) -> Rgb {
    let fy = (lab.l + 16.0) / 116.0;
    let fx = lab.a / 500.0 + fy;
    let fz = fy - lab.b / 200.0;

    let x = XN * lab_f_inv(fx);
    let y = YN * lab_f_inv(fy);
    let z = ZN * lab_f_inv(fz);

    // XYZ → linear RGB (D65)
    let r = 3.2404542 * x - 1.5371385 * y - 0.4985314 * z;
    let g = -0.9692660 * x + 1.8760108 * y + 0.0415560 * z;
    let b = 0.0556434 * x - 0.2040259 * y + 1.0572252 * z;

    Rgb(linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b))
}

/// Linearly interpolate between two LAB colors.
fn lerp_lab(t: f64, a: &Lab, b: &Lab) -> Lab {
    Lab {
        l: a.l + t * (b.l - a.l),
        a: a.a + t * (b.a - a.a),
        b: a.b + t * (b.b - a.b),
    }
}

// ─── CubeCoord ──────────────────────────────────────────────────────────────

/// A position in the theme-relative color cube.
///
/// Each axis ranges from `0.0` to `1.0`, representing a fractional position
/// between theme anchor colors:
///
/// - **r**: red axis — interpolates bg→red, green→yellow, blue→magenta, cyan→fg
/// - **g**: green axis — interpolates between the r-axis edge pairs
/// - **b**: blue axis — interpolates between the g-axis face results
///
/// Designers think in percentages: `cube(60%, 20%, 0%)` maps to
/// `CubeCoord { r: 0.6, g: 0.2, b: 0.0 }`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubeCoord {
    /// Red axis fraction (0.0–1.0).
    pub r: f64,
    /// Green axis fraction (0.0–1.0).
    pub g: f64,
    /// Blue axis fraction (0.0–1.0).
    pub b: f64,
}

impl CubeCoord {
    /// Creates a new cube coordinate with fractional values (0.0–1.0 per axis).
    ///
    /// Returns an error if any component is outside the valid range.
    pub fn new(r: f64, g: f64, b: f64) -> Result<Self, String> {
        if !(0.0..=1.0).contains(&r) || !(0.0..=1.0).contains(&g) || !(0.0..=1.0).contains(&b) {
            return Err(format!(
                "CubeCoord components must be 0.0..=1.0, got ({}, {}, {})",
                r, g, b
            ));
        }
        Ok(Self { r, g, b })
    }

    /// Creates a cube coordinate from percentage values (0.0–100.0 per axis).
    ///
    /// This is the natural syntax for style definitions: `cube(60%, 20%, 0%)`
    /// maps to `from_percentages(60.0, 20.0, 0.0)`.
    pub fn from_percentages(r: f64, g: f64, b: f64) -> Result<Self, String> {
        Self::new(r / 100.0, g / 100.0, b / 100.0)
    }

    /// Quantizes this coordinate to the nearest grid point for a given number
    /// of subdivisions per axis.
    ///
    /// For the standard 256-color palette, `levels = 6` (producing a 6×6×6 cube).
    /// Returns the integer grid coordinates `(r, g, b)` each in `0..levels`.
    pub fn quantize(&self, levels: u8) -> (u8, u8, u8) {
        let max = (levels - 1) as f64;
        let r = (self.r * max).round() as u8;
        let g = (self.g * max).round() as u8;
        let b = (self.b * max).round() as u8;
        (r.min(levels - 1), g.min(levels - 1), b.min(levels - 1))
    }

    /// Returns the 256-color palette index for this coordinate.
    ///
    /// Uses the standard formula: `16 + 36*r + 6*g + b` where `r`, `g`, `b`
    /// are quantized to `0..levels`. The offset of 16 accounts for the base16
    /// ANSI colors that occupy indices 0–15.
    pub fn to_palette_index(&self, levels: u8) -> u8 {
        let (r, g, b) = self.quantize(levels);
        let levels_sq = levels as u16 * levels as u16;
        (16 + levels_sq * r as u16 + levels as u16 * g as u16 + b as u16) as u8
    }
}

// ─── ThemePalette ───────────────────────────────────────────────────────────

/// A set of 8 anchor colors that define a theme-relative color space.
///
/// The 8 anchors map to the standard ANSI color positions:
///
/// | Index | Color   | Cube corner     |
/// |-------|---------|-----------------|
/// | 0     | black   | `(0, 0, 0)`     |
/// | 1     | red     | `(1, 0, 0)`     |
/// | 2     | green   | `(0, 1, 0)`     |
/// | 3     | yellow  | `(1, 1, 0)`     |
/// | 4     | blue    | `(0, 0, 1)`     |
/// | 5     | magenta | `(1, 0, 1)`     |
/// | 6     | cyan    | `(0, 1, 1)`     |
/// | 7     | white   | `(1, 1, 1)`     |
///
/// Optional background/foreground overrides let the bg/fg differ from the
/// theme's black/white (e.g., Solarized where bg is a dark teal, not black).
#[derive(Debug, Clone)]
pub struct ThemePalette {
    anchors: [Rgb; 8],
    bg: Rgb,
    fg: Rgb,
}

impl ThemePalette {
    /// Creates a new palette from the 8 base ANSI colors.
    ///
    /// The array order must be: black, red, green, yellow, blue, magenta, cyan, white.
    /// Background defaults to `anchors[0]` (black) and foreground to `anchors[7]` (white).
    pub fn new(anchors: [Rgb; 8]) -> Self {
        let bg = anchors[0];
        let fg = anchors[7];
        Self { anchors, bg, fg }
    }

    /// Overrides the background color used for the `(0,0,0)` cube corner.
    ///
    /// Useful for themes where the terminal background differs from ANSI black
    /// (e.g., Solarized Dark uses `#002b36`).
    pub fn with_bg(mut self, bg: Rgb) -> Self {
        self.bg = bg;
        self
    }

    /// Overrides the foreground color used for the `(1,1,1)` cube corner.
    ///
    /// Useful for themes where the terminal foreground differs from ANSI white
    /// (e.g., Solarized Dark uses `#fdf6e3`).
    pub fn with_fg(mut self, fg: Rgb) -> Self {
        self.fg = fg;
        self
    }

    /// Resolves a [`CubeCoord`] to an actual RGB color via trilinear LAB interpolation.
    ///
    /// This is the core operation: given a position in the theme cube, compute
    /// the perceptually interpolated color between the 8 anchor corners.
    pub fn resolve(&self, coord: &CubeCoord) -> Rgb {
        let bg_lab = rgb_to_lab(self.bg);
        let fg_lab = rgb_to_lab(self.fg);
        let labs: Vec<Lab> = self.anchors.iter().map(|c| rgb_to_lab(*c)).collect();

        // R-axis: interpolate 4 edge pairs
        let c0 = lerp_lab(coord.r, &bg_lab, &labs[1]); // bg → red
        let c1 = lerp_lab(coord.r, &labs[2], &labs[3]); // green → yellow
        let c2 = lerp_lab(coord.r, &labs[4], &labs[5]); // blue → magenta
        let c3 = lerp_lab(coord.r, &labs[6], &fg_lab); // cyan → fg

        // G-axis: interpolate between edge pairs
        let c4 = lerp_lab(coord.g, &c0, &c1);
        let c5 = lerp_lab(coord.g, &c2, &c3);

        // B-axis: interpolate between faces
        let c6 = lerp_lab(coord.b, &c4, &c5);

        lab_to_rgb(c6)
    }

    /// Generates the extended color palette by subdividing the theme cube.
    ///
    /// Returns a `Vec<Rgb>` containing:
    /// - `subdivisions³` colors from the cube (e.g., 216 for subdivisions=6)
    /// - 24 grayscale colors interpolated from background to foreground
    ///
    /// The total is `subdivisions³ + 24` colors. For subdivisions=6, this gives
    /// the 240 extended colors that fill indices 16–255 of a 256-color palette.
    pub fn generate_palette(&self, subdivisions: u8) -> Vec<Rgb> {
        let bg_lab = rgb_to_lab(self.bg);
        let fg_lab = rgb_to_lab(self.fg);
        let labs: Vec<Lab> = self.anchors.iter().map(|c| rgb_to_lab(*c)).collect();
        let max = (subdivisions - 1) as f64;

        let mut palette = Vec::new();

        // Color cube
        for r in 0..subdivisions {
            let rt = r as f64 / max;
            let c0 = lerp_lab(rt, &bg_lab, &labs[1]);
            let c1 = lerp_lab(rt, &labs[2], &labs[3]);
            let c2 = lerp_lab(rt, &labs[4], &labs[5]);
            let c3 = lerp_lab(rt, &labs[6], &fg_lab);

            for g in 0..subdivisions {
                let gt = g as f64 / max;
                let c4 = lerp_lab(gt, &c0, &c1);
                let c5 = lerp_lab(gt, &c2, &c3);

                for b in 0..subdivisions {
                    let bt = b as f64 / max;
                    let c6 = lerp_lab(bt, &c4, &c5);
                    palette.push(lab_to_rgb(c6));
                }
            }
        }

        // Grayscale ramp (24 steps between bg and fg, exclusive of endpoints)
        for i in 0..24 {
            let t = (i + 1) as f64 / 25.0;
            let lab = lerp_lab(t, &bg_lab, &fg_lab);
            palette.push(lab_to_rgb(lab));
        }

        palette
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // LAB round-trip tests
    // =====================================================================

    /// Assert that RGB → LAB → RGB round-trips within tolerance.
    fn assert_rgb_roundtrip(rgb: Rgb, tolerance: u8) {
        let lab = rgb_to_lab(rgb);
        let back = lab_to_rgb(lab);
        let dr = (rgb.0 as i16 - back.0 as i16).unsigned_abs() as u8;
        let dg = (rgb.1 as i16 - back.1 as i16).unsigned_abs() as u8;
        let db = (rgb.2 as i16 - back.2 as i16).unsigned_abs() as u8;
        assert!(
            dr <= tolerance && dg <= tolerance && db <= tolerance,
            "Round-trip failed: {:?} → {:?} → {:?} (delta: {}, {}, {})",
            rgb,
            lab,
            back,
            dr,
            dg,
            db
        );
    }

    #[test]
    fn roundtrip_black() {
        assert_rgb_roundtrip(Rgb(0, 0, 0), 1);
    }

    #[test]
    fn roundtrip_white() {
        assert_rgb_roundtrip(Rgb(255, 255, 255), 1);
    }

    #[test]
    fn roundtrip_pure_red() {
        assert_rgb_roundtrip(Rgb(255, 0, 0), 1);
    }

    #[test]
    fn roundtrip_pure_green() {
        assert_rgb_roundtrip(Rgb(0, 255, 0), 1);
    }

    #[test]
    fn roundtrip_pure_blue() {
        assert_rgb_roundtrip(Rgb(0, 0, 255), 1);
    }

    #[test]
    fn roundtrip_mid_gray() {
        assert_rgb_roundtrip(Rgb(128, 128, 128), 1);
    }

    #[test]
    fn roundtrip_arbitrary_color() {
        assert_rgb_roundtrip(Rgb(200, 100, 50), 1);
    }

    // =====================================================================
    // Known LAB values
    // =====================================================================

    #[test]
    fn lab_black_is_zero_lightness() {
        let lab = rgb_to_lab(Rgb(0, 0, 0));
        assert!(lab.l.abs() < 1.0, "Black L* should be ~0, got {}", lab.l);
    }

    #[test]
    fn lab_white_is_full_lightness() {
        let lab = rgb_to_lab(Rgb(255, 255, 255));
        assert!(
            (lab.l - 100.0).abs() < 1.0,
            "White L* should be ~100, got {}",
            lab.l
        );
    }

    #[test]
    fn lab_red_has_positive_a() {
        let lab = rgb_to_lab(Rgb(255, 0, 0));
        assert!(
            lab.a > 50.0,
            "Red should have large positive a*, got {}",
            lab.a
        );
    }

    // =====================================================================
    // lerp_lab tests
    // =====================================================================

    #[test]
    fn lerp_at_zero_returns_first() {
        let a = rgb_to_lab(Rgb(255, 0, 0));
        let b = rgb_to_lab(Rgb(0, 0, 255));
        let result = lerp_lab(0.0, &a, &b);
        assert!((result.l - a.l).abs() < 0.001);
        assert!((result.a - a.a).abs() < 0.001);
        assert!((result.b - a.b).abs() < 0.001);
    }

    #[test]
    fn lerp_at_one_returns_second() {
        let a = rgb_to_lab(Rgb(255, 0, 0));
        let b = rgb_to_lab(Rgb(0, 0, 255));
        let result = lerp_lab(1.0, &a, &b);
        assert!((result.l - b.l).abs() < 0.001);
        assert!((result.a - b.a).abs() < 0.001);
        assert!((result.b - b.b).abs() < 0.001);
    }

    #[test]
    fn lerp_midpoint_is_between() {
        let a = rgb_to_lab(Rgb(0, 0, 0));
        let b = rgb_to_lab(Rgb(255, 255, 255));
        let mid = lerp_lab(0.5, &a, &b);
        assert!(mid.l > a.l && mid.l < b.l);
    }

    // =====================================================================
    // CubeCoord validation tests
    // =====================================================================

    #[test]
    fn cubecoord_valid_range() {
        assert!(CubeCoord::new(0.0, 0.0, 0.0).is_ok());
        assert!(CubeCoord::new(1.0, 1.0, 1.0).is_ok());
        assert!(CubeCoord::new(0.5, 0.5, 0.5).is_ok());
    }

    #[test]
    fn cubecoord_rejects_negative() {
        assert!(CubeCoord::new(-0.1, 0.0, 0.0).is_err());
        assert!(CubeCoord::new(0.0, -0.1, 0.0).is_err());
        assert!(CubeCoord::new(0.0, 0.0, -0.1).is_err());
    }

    #[test]
    fn cubecoord_rejects_over_one() {
        assert!(CubeCoord::new(1.1, 0.0, 0.0).is_err());
        assert!(CubeCoord::new(0.0, 1.1, 0.0).is_err());
        assert!(CubeCoord::new(0.0, 0.0, 1.1).is_err());
    }

    #[test]
    fn cubecoord_from_percentages() {
        let coord = CubeCoord::from_percentages(60.0, 20.0, 0.0).unwrap();
        assert!((coord.r - 0.6).abs() < 0.001);
        assert!((coord.g - 0.2).abs() < 0.001);
        assert!((coord.b - 0.0).abs() < 0.001);
    }

    #[test]
    fn cubecoord_from_percentages_bounds() {
        assert!(CubeCoord::from_percentages(0.0, 0.0, 0.0).is_ok());
        assert!(CubeCoord::from_percentages(100.0, 100.0, 100.0).is_ok());
        assert!(CubeCoord::from_percentages(101.0, 0.0, 0.0).is_err());
        assert!(CubeCoord::from_percentages(-1.0, 0.0, 0.0).is_err());
    }

    // =====================================================================
    // CubeCoord quantization tests
    // =====================================================================

    #[test]
    fn quantize_corners_levels_6() {
        assert_eq!(
            CubeCoord::new(0.0, 0.0, 0.0).unwrap().quantize(6),
            (0, 0, 0)
        );
        assert_eq!(
            CubeCoord::new(1.0, 1.0, 1.0).unwrap().quantize(6),
            (5, 5, 5)
        );
        assert_eq!(
            CubeCoord::new(1.0, 0.0, 0.0).unwrap().quantize(6),
            (5, 0, 0)
        );
        assert_eq!(
            CubeCoord::new(0.0, 1.0, 0.0).unwrap().quantize(6),
            (0, 5, 0)
        );
        assert_eq!(
            CubeCoord::new(0.0, 0.0, 1.0).unwrap().quantize(6),
            (0, 0, 5)
        );
    }

    #[test]
    fn quantize_midpoint_levels_6() {
        // 0.5 * 5 = 2.5, rounds to 3 (standard rounding, but actually let's check)
        // Actually: 0.5 * 5.0 = 2.5, round() = 3 in Rust (round half to even? no, round half away from zero)
        // f64::round(2.5) = 3.0 in Rust
        let (r, g, b) = CubeCoord::new(0.5, 0.5, 0.5).unwrap().quantize(6);
        assert_eq!((r, g, b), (3, 3, 3));
    }

    #[test]
    fn quantize_one_fifth_levels_6() {
        // 0.2 * 5 = 1.0, rounds to 1
        let (r, _, _) = CubeCoord::new(0.2, 0.0, 0.0).unwrap().quantize(6);
        assert_eq!(r, 1);
    }

    // =====================================================================
    // to_palette_index tests
    // =====================================================================

    #[test]
    fn palette_index_origin() {
        // (0,0,0) → 16 + 0 = 16
        assert_eq!(
            CubeCoord::new(0.0, 0.0, 0.0).unwrap().to_palette_index(6),
            16
        );
    }

    #[test]
    fn palette_index_max() {
        // (5,5,5) → 16 + 36*5 + 6*5 + 5 = 16 + 180 + 30 + 5 = 231
        assert_eq!(
            CubeCoord::new(1.0, 1.0, 1.0).unwrap().to_palette_index(6),
            231
        );
    }

    #[test]
    fn palette_index_pure_red() {
        // (5,0,0) → 16 + 36*5 = 16 + 180 = 196
        assert_eq!(
            CubeCoord::new(1.0, 0.0, 0.0).unwrap().to_palette_index(6),
            196
        );
    }

    #[test]
    fn palette_index_pure_blue() {
        // (0,0,5) → 16 + 5 = 21
        assert_eq!(
            CubeCoord::new(0.0, 0.0, 1.0).unwrap().to_palette_index(6),
            21
        );
    }

    #[test]
    fn palette_index_pure_green() {
        // (0,5,0) → 16 + 30 = 46
        assert_eq!(
            CubeCoord::new(0.0, 1.0, 0.0).unwrap().to_palette_index(6),
            46
        );
    }

    // =====================================================================
    // ThemePalette resolve tests
    // =====================================================================

    /// Standard xterm-like base colors for testing.
    fn test_palette() -> ThemePalette {
        ThemePalette::new([
            Rgb(0, 0, 0),       // black
            Rgb(205, 0, 0),     // red
            Rgb(0, 205, 0),     // green
            Rgb(205, 205, 0),   // yellow
            Rgb(0, 0, 238),     // blue
            Rgb(205, 0, 205),   // magenta
            Rgb(0, 205, 205),   // cyan
            Rgb(229, 229, 229), // white
        ])
    }

    #[test]
    fn resolve_corner_bg() {
        let palette = test_palette();
        let coord = CubeCoord::new(0.0, 0.0, 0.0).unwrap();
        let rgb = palette.resolve(&coord);
        assert_eq!(rgb, Rgb(0, 0, 0));
    }

    #[test]
    fn resolve_corner_red() {
        let palette = test_palette();
        let coord = CubeCoord::new(1.0, 0.0, 0.0).unwrap();
        let rgb = palette.resolve(&coord);
        assert_eq!(rgb, Rgb(205, 0, 0));
    }

    #[test]
    fn resolve_corner_green() {
        let palette = test_palette();
        let coord = CubeCoord::new(0.0, 1.0, 0.0).unwrap();
        let rgb = palette.resolve(&coord);
        assert_eq!(rgb, Rgb(0, 205, 0));
    }

    #[test]
    fn resolve_corner_yellow() {
        let palette = test_palette();
        let coord = CubeCoord::new(1.0, 1.0, 0.0).unwrap();
        let rgb = palette.resolve(&coord);
        assert_eq!(rgb, Rgb(205, 205, 0));
    }

    #[test]
    fn resolve_corner_blue() {
        let palette = test_palette();
        let coord = CubeCoord::new(0.0, 0.0, 1.0).unwrap();
        let rgb = palette.resolve(&coord);
        assert_eq!(rgb, Rgb(0, 0, 238));
    }

    #[test]
    fn resolve_corner_magenta() {
        let palette = test_palette();
        let coord = CubeCoord::new(1.0, 0.0, 1.0).unwrap();
        let rgb = palette.resolve(&coord);
        assert_eq!(rgb, Rgb(205, 0, 205));
    }

    #[test]
    fn resolve_corner_cyan() {
        let palette = test_palette();
        let coord = CubeCoord::new(0.0, 1.0, 1.0).unwrap();
        let rgb = palette.resolve(&coord);
        assert_eq!(rgb, Rgb(0, 205, 205));
    }

    #[test]
    fn resolve_corner_fg() {
        let palette = test_palette();
        let coord = CubeCoord::new(1.0, 1.0, 1.0).unwrap();
        let rgb = palette.resolve(&coord);
        assert_eq!(rgb, Rgb(229, 229, 229));
    }

    #[test]
    fn resolve_center_is_blend() {
        let palette = test_palette();
        let coord = CubeCoord::new(0.5, 0.5, 0.5).unwrap();
        let rgb = palette.resolve(&coord);
        // Center should not be any corner color
        assert_ne!(rgb, Rgb(0, 0, 0));
        assert_ne!(rgb, Rgb(255, 255, 255));
        // Should be somewhere in the middle range
        assert!(rgb.0 > 50 && rgb.0 < 200);
        assert!(rgb.1 > 50 && rgb.1 < 200);
        assert!(rgb.2 > 50 && rgb.2 < 200);
    }

    #[test]
    fn resolve_with_custom_bg_fg() {
        let palette = test_palette()
            .with_bg(Rgb(30, 30, 46))
            .with_fg(Rgb(205, 214, 244));

        let origin = palette.resolve(&CubeCoord::new(0.0, 0.0, 0.0).unwrap());
        assert_eq!(origin, Rgb(30, 30, 46));

        let corner = palette.resolve(&CubeCoord::new(1.0, 1.0, 1.0).unwrap());
        assert_eq!(corner, Rgb(205, 214, 244));
    }

    // =====================================================================
    // generate_palette tests
    // =====================================================================

    #[test]
    fn generate_palette_correct_count() {
        let palette = test_palette();
        let extended = palette.generate_palette(6);
        // 6^3 = 216 cube colors + 24 grayscale = 240
        assert_eq!(extended.len(), 240);
    }

    #[test]
    fn generate_palette_first_entry_is_bg() {
        let palette = test_palette();
        let extended = palette.generate_palette(6);
        assert_eq!(extended[0], Rgb(0, 0, 0));
    }

    #[test]
    fn generate_palette_last_cube_entry_is_fg() {
        let palette = test_palette();
        let extended = palette.generate_palette(6);
        // Last cube entry is index 215 (6^3 - 1), which is (5,5,5) = fg
        assert_eq!(extended[215], Rgb(229, 229, 229));
    }

    #[test]
    fn generate_palette_red_corner() {
        let palette = test_palette();
        let extended = palette.generate_palette(6);
        // Red corner is (5,0,0) → index 5*36 = 180
        assert_eq!(extended[180], Rgb(205, 0, 0));
    }

    #[test]
    fn generate_palette_grayscale_monotonic_lightness() {
        let palette = test_palette();
        let extended = palette.generate_palette(6);
        // Grayscale ramp is the last 24 entries
        let grayscale = &extended[216..240];

        for i in 1..grayscale.len() {
            let prev_l = rgb_to_lab(grayscale[i - 1]).l;
            let curr_l = rgb_to_lab(grayscale[i]).l;
            assert!(
                curr_l >= prev_l - 0.01,
                "Grayscale lightness not monotonic at index {}: {} < {}",
                i,
                curr_l,
                prev_l
            );
        }
    }

    #[test]
    fn generate_palette_different_subdivisions() {
        let palette = test_palette();
        // 4^3 = 64 + 24 = 88
        let small = palette.generate_palette(4);
        assert_eq!(small.len(), 88);
        // 8^3 = 512 + 24 = 536
        let large = palette.generate_palette(8);
        assert_eq!(large.len(), 536);
    }

    #[test]
    fn generate_palette_with_gruvbox() {
        let palette = ThemePalette::new([
            Rgb(40, 40, 40),    // black
            Rgb(204, 36, 29),   // red
            Rgb(152, 151, 26),  // green
            Rgb(215, 153, 33),  // yellow
            Rgb(69, 133, 136),  // blue
            Rgb(177, 98, 134),  // magenta
            Rgb(104, 157, 106), // cyan
            Rgb(168, 153, 132), // white
        ])
        .with_bg(Rgb(40, 40, 40))
        .with_fg(Rgb(235, 219, 178));

        let extended = palette.generate_palette(6);
        assert_eq!(extended.len(), 240);

        // bg corner should match
        assert_eq!(extended[0], Rgb(40, 40, 40));
        // fg corner should match
        assert_eq!(extended[215], Rgb(235, 219, 178));
    }
}
