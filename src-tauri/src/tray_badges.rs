//! Tray / menu-bar badge icons: the provider logo with one thin usage arc.
//!
//! Minimal on purpose: no thick rings and no glow. A ~1.5px (at 16px) slate
//! track carries the primary window's usage as an arc. The arc stays neutral
//! while idle, takes a soft accent only while that provider has an active
//! session, and turns a muted red near the limit. The exact figure lives in
//! the tooltip ("Claude 58% (Resets in 4h)"). The background is transparent,
//! so the icon sits naturally on light and dark Windows 11 taskbars.
//! Rendered on the CPU into a small RGBA buffer: no font or vector dependency.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

use crate::usage::models::ProviderId;

/// Rendered edge length in pixels (2x the ~16pt menu-bar icon slot).
pub const BADGE_SIZE: u32 = 44;
/// Horizontal gap between badges in the strip, in pixels.
pub const STRIP_GAP: u32 = 8;

/// Arc and logo geometry, in pixels of the `BADGE_SIZE` canvas.
#[derive(Debug, Clone, Copy)]
pub struct BadgeLayout {
    /// Inner/outer radius of the usage band.
    ring: (f32, f32),
    logo_radius: f32,
    /// Track opacity: low, but visible on both light and dark taskbars.
    track_alpha: f32,
}

/// macOS menu bar (rendered at 44px, shown at ~22pt).
pub const MENU_BAR_LAYOUT: BadgeLayout = BadgeLayout {
    ring: (19.4, 22.0),
    logo_radius: 16.4,
    track_alpha: 0.24,
};

/// Windows notification area: downscaled to a 16px slot (at 100% scaling),
/// where the 4.1px band becomes ~1.5px. The logo fills most of the slot so it
/// reads at the same size as the neighbouring icons.
pub const TRAY_LAYOUT: BadgeLayout = BadgeLayout {
    ring: (17.9, 22.0),
    logo_radius: 15.6,
    track_alpha: 0.28,
};

const SLATE_TRACK: [u8; 3] = [0x94, 0xa3, 0xb8]; // slate-400
const ARC_IDLE: [u8; 3] = [0x94, 0xa3, 0xb8]; // slate-400: readable on light and dark
const ARC_ACTIVE: [u8; 3] = [0x5e, 0xc8, 0xa0]; // softened emerald
const ARC_LIMIT: [u8; 3] = [0xe0, 0x7a, 0x7a]; // muted red

fn logo_bytes(provider: ProviderId) -> &'static [u8] {
    match provider {
        ProviderId::Claude => include_bytes!("../../public/providers/claude-ai-icon.png"),
        ProviderId::Codex => include_bytes!("../../public/providers/chatgpt-icon.png"),
        ProviderId::Antigravity => include_bytes!("../../public/providers/antigravity-icon.png"),
        ProviderId::Cursor => include_bytes!("../../public/providers/cursor-icon.png"),
    }
}

/// The logo decoded and scaled to a `side`-px disc, cached per size.
fn logo(provider: ProviderId, side: u32) -> Option<RgbaImage> {
    static LOGOS: OnceLock<Mutex<HashMap<(ProviderId, u32), RgbaImage>>> = OnceLock::new();
    let mut cache = LOGOS.get_or_init(Default::default).lock().ok()?;
    if let Some(img) = cache.get(&(provider, side)) {
        return Some(img.clone());
    }
    let img = image::load_from_memory_with_format(logo_bytes(provider), image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let scaled = fit_square(&img, side);
    cache.insert((provider, side), scaled.clone());
    Some(scaled)
}

/// Scale into a `side`x`side` square, preserving aspect ratio, centred.
fn fit_square(img: &RgbaImage, side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let scale = side as f32 / w.max(h) as f32;
    let (nw, nh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    let resized = imageops::resize(img, nw, nh, FilterType::Lanczos3);
    let mut out = RgbaImage::new(side, side);
    imageops::overlay(
        &mut out,
        &resized,
        ((side - nw) / 2) as i64,
        ((side - nh) / 2) as i64,
    );
    out
}

/// Arc colour: neutral by default, a soft accent while the provider has an
/// active session, muted red from 90%.
pub fn arc_color(pct: f64, active: bool) -> [u8; 3] {
    if pct >= 90.0 {
        ARC_LIMIT
    } else if active {
        ARC_ACTIVE
    } else {
        ARC_IDLE
    }
}

/// Source-over blend of `color` at `alpha` (0..1) onto `px`.
fn blend(px: &mut Rgba<u8>, color: [u8; 3], alpha: f32) {
    if alpha <= 0.0 {
        return;
    }
    let src_a = alpha.min(1.0);
    let dst_a = px[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    for i in 0..3 {
        let c = (color[i] as f32 * src_a + px[i] as f32 * dst_a * (1.0 - src_a)) / out_a.max(1e-6);
        px[i] = c.round().clamp(0.0, 255.0) as u8;
    }
    px[3] = (out_a * 255.0).round() as u8;
}

/// Coverage (0..1) of a pixel at radius `r` inside the band `[inner, outer]`,
/// with a 1px anti-aliased edge.
fn band_coverage(r: f32, (inner, outer): (f32, f32)) -> f32 {
    ((r - inner + 0.5).clamp(0.0, 1.0)).min((outer - r + 0.5).clamp(0.0, 1.0))
}

/// Draw the faint slate track, then the progress arc clockwise from the top.
fn draw_ring(img: &mut RgbaImage, band: (f32, f32), track_alpha: f32, pct: f64, active: bool) {
    let c = BADGE_SIZE as f32 / 2.0;
    let color = arc_color(pct, active);
    let fraction = (pct.clamp(0.0, 100.0) / 100.0) as f32;
    let sweep = fraction * std::f32::consts::TAU;
    let mid_r = (band.0 + band.1) / 2.0;
    for (x, y, px) in img.enumerate_pixels_mut() {
        let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
        let r = (dx * dx + dy * dy).sqrt();
        let coverage = band_coverage(r, band);
        if coverage <= 0.0 {
            continue;
        }
        blend(px, SLATE_TRACK, coverage * track_alpha);
        if fraction <= 0.0 {
            continue;
        }
        // Angle clockwise from the top, 0..TAU.
        let angle = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU);
        let arc = if fraction >= 1.0 {
            1.0
        } else {
            // Soften the arc's end over ~1px of arc length.
            ((sweep - angle) * mid_r + 0.5).clamp(0.0, 1.0)
        };
        blend(px, color, coverage * arc);
    }
}

/// Composite the logo, masked to a circle, at the badge centre.
fn draw_logo(img: &mut RgbaImage, provider: ProviderId, radius: f32) {
    let Some(logo) = logo(provider, (radius * 2.0).round() as u32) else {
        return;
    };
    let c = BADGE_SIZE as f32 / 2.0;
    let offset = (c - radius).round() as u32;
    for (lx, ly, src) in logo.enumerate_pixels() {
        let (x, y) = (lx + offset, ly + offset);
        if x >= BADGE_SIZE || y >= BADGE_SIZE {
            continue;
        }
        let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
        let mask = (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
        let alpha = src[3] as f32 / 255.0 * mask;
        blend(img.get_pixel_mut(x, y), [src[0], src[1], src[2]], alpha);
    }
}

/// Render the badge for `primary` % usage.
pub fn render_badge(provider: ProviderId, primary: f64, active: bool) -> RgbaImage {
    render_badge_with(MENU_BAR_LAYOUT, provider, primary, active)
}

fn render_badge_with(
    layout: BadgeLayout,
    provider: ProviderId,
    primary: f64,
    active: bool,
) -> RgbaImage {
    let mut img = RgbaImage::new(BADGE_SIZE, BADGE_SIZE);
    draw_ring(&mut img, layout.ring, layout.track_alpha, primary, active);
    draw_logo(&mut img, provider, layout.logo_radius);
    img
}

/// A badge for the Windows notification area, downscaled here to the slot's
/// exact `size` px: the shell's own resampling of the 44px badge is coarse
/// and leaves the arc jagged.
pub fn render_tray_badge(provider: ProviderId, primary: f64, active: bool, size: u32) -> RgbaImage {
    let img = render_badge_with(TRAY_LAYOUT, provider, primary, active);
    if size == 0 || size >= BADGE_SIZE {
        return img;
    }
    imageops::resize(&img, size, size, FilterType::Lanczos3)
}

/// Lay `badges` (provider, primary %, active) out left to right in one image,
/// `BADGE_SIZE` tall.
pub fn render_strip(badges: &[(ProviderId, f64, bool)]) -> RgbaImage {
    let n = badges.len() as u32;
    let width = (n * BADGE_SIZE + n.saturating_sub(1) * STRIP_GAP).max(1);
    let mut strip = RgbaImage::new(width, BADGE_SIZE);
    for (i, (provider, primary, active)) in badges.iter().enumerate() {
        let badge = render_badge(*provider, *primary, *active);
        let x = i as u32 * (BADGE_SIZE + STRIP_GAP);
        imageops::overlay(&mut strip, &badge, x as i64, 0);
    }
    strip
}

/// Which badge a horizontal position falls on, given the strip's on-screen
/// width. Clicks in a gap snap to the nearer badge.
pub fn badge_index_at(fraction: f64, count: usize) -> Option<usize> {
    if count == 0 || !fraction.is_finite() {
        return None;
    }
    Some(((fraction.clamp(0.0, 0.999_999)) * count as f64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(img: &RgbaImage, x: u32, y: u32) -> u8 {
        img.get_pixel(x, y)[3]
    }

    /// Pixel on the band's centre line at `deg` clockwise from the top.
    fn ring_px(img: &RgbaImage, deg: f32) -> Rgba<u8> {
        let c = BADGE_SIZE as f32 / 2.0;
        let (inner, outer) = MENU_BAR_LAYOUT.ring;
        let r = (inner + outer) / 2.0;
        let a = deg.to_radians();
        let (x, y) = (c + r * a.sin(), c - r * a.cos());
        *img.get_pixel(x as u32, y as u32)
    }

    #[test]
    fn arc_is_neutral_unless_active_or_near_the_limit() {
        assert_eq!(arc_color(40.0, false), ARC_IDLE);
        assert_eq!(arc_color(40.0, true), ARC_ACTIVE);
        assert_eq!(arc_color(95.0, false), ARC_LIMIT);
    }

    #[test]
    fn progress_arc_covers_only_its_share() {
        let img = render_badge(ProviderId::Claude, 25.0, false);
        let filled = ring_px(&img, 45.0);
        assert!(filled[3] > 200, "arc alpha {}", filled[3]);
        let track = ring_px(&img, 180.0);
        assert!(track[3] < 110, "track alpha {}", track[3]);
    }

    #[test]
    fn corners_stay_transparent_and_logo_is_drawn() {
        for provider in ProviderId::all() {
            let img = render_badge(provider, 60.0, true);
            assert_eq!(img.dimensions(), (BADGE_SIZE, BADGE_SIZE));
            assert_eq!(alpha_at(&img, 0, 0), 0);
            assert_eq!(alpha_at(&img, BADGE_SIZE - 1, BADGE_SIZE - 1), 0);
            let c = BADGE_SIZE / 2;
            assert!(
                alpha_at(&img, c, c) > 0,
                "{} logo missing",
                provider.as_str()
            );
        }
    }

    #[test]
    fn full_ring_is_closed() {
        let img = render_badge(ProviderId::Codex, 100.0, false);
        for deg in [0.0, 90.0, 180.0, 270.0, 359.0] {
            assert!(ring_px(&img, deg)[3] > 200, "gap at {deg} degrees");
        }
    }

    #[test]
    fn strip_places_badges_side_by_side() {
        let strip = render_strip(&[
            (ProviderId::Claude, 20.0, true),
            (ProviderId::Codex, 0.0, false),
            (ProviderId::Antigravity, 0.0, false),
        ]);
        assert_eq!(
            strip.dimensions(),
            (3 * BADGE_SIZE + 2 * STRIP_GAP, BADGE_SIZE)
        );
        let mid = BADGE_SIZE / 2;
        for i in 0..3 {
            let cx = i * (BADGE_SIZE + STRIP_GAP) + mid;
            assert!(strip.get_pixel(cx, mid)[3] > 0, "badge {i} missing");
        }
        // The gap between badges stays transparent.
        assert_eq!(strip.get_pixel(BADGE_SIZE + STRIP_GAP / 2, mid)[3], 0);
    }

    #[test]
    fn tray_badge_is_slot_sized() {
        let img = render_tray_badge(ProviderId::Claude, 40.0, false, 16);
        assert_eq!(img.dimensions(), (16, 16));
        assert!(alpha_at(&img, 8, 8) > 0);
    }

    // The band never overlaps the logo.
    const _: () = assert!(TRAY_LAYOUT.ring.0 > TRAY_LAYOUT.logo_radius);
    const _: () = assert!(MENU_BAR_LAYOUT.ring.0 > MENU_BAR_LAYOUT.logo_radius);

    #[test]
    fn tray_band_is_about_one_and_a_half_px_at_16px() {
        let band = (TRAY_LAYOUT.ring.1 - TRAY_LAYOUT.ring.0) * 16.0 / BADGE_SIZE as f32;
        assert!((1.3..=1.7).contains(&band), "band {band}px");
    }

    #[test]
    fn click_position_maps_to_badge() {
        assert_eq!(badge_index_at(0.1, 3), Some(0));
        assert_eq!(badge_index_at(0.5, 3), Some(1));
        assert_eq!(badge_index_at(1.0, 3), Some(2));
        assert_eq!(badge_index_at(-0.2, 3), Some(0));
        assert_eq!(badge_index_at(0.5, 0), None);
    }
}
