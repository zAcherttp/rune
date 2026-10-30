//! How a runtime's colours blend and its glyphs rasterise: sRGB or linear-light blending,
//! the font smoothing mode, and the per-glyph instances they shape.

use super::*;
use crate::text::FontSmoothing;

impl<S: 'static> Runtime<S> {
    /// Enable or disable gamma-space (sRGB) blending for translucent fills, shadows, blurs,
    /// images and video, as `App::srgb_blend` does for a window. Every colour the next frame
    /// builds, hover and press tints included, is premultiplied in sRGB encoding
    /// (`Color::premul_srgb`). A runtime drawn by the app itself (`Runtime::with_text`, a
    /// headless snapshot) then draws with `Renderer::set_srgb_blend(true)` into a non-sRGB
    /// target (`format.remove_srgb_suffix()`), cleared to `clear_color()`.
    pub fn set_srgb_blend(&mut self, enabled: bool) {
        if self.srgb_blend != enabled {
            self.srgb_blend = enabled;
            self.dirty = true;
        }
    }

    pub fn srgb_blend(&self) -> bool {
        self.srgb_blend
    }

    /// Rasterise and place glyphs in `mode` from the next frame, as `App::font_smoothing` does
    /// for a window. Glyphs already rasterised in another mode stay cached for a switch back.
    /// Blending is separate: both modes match CoreText with [`Self::set_srgb_blend`] on.
    pub fn set_font_smoothing(&mut self, mode: FontSmoothing) {
        if self.text.smoothing != mode {
            self.text.set_font_smoothing(mode);
            self.dirty = true;
        }
    }

    pub fn font_smoothing(&self) -> FontSmoothing {
        self.text.smoothing
    }

    pub fn clear_color(&self) -> [f32; 4] {
        if self.transparent { return [0.0; 4]; }
        self.backdrop_fill().map_or([0.0, 0.0, 0.0, 1.0], |c| {
            if self.srgb_blend {
                [srgb_encode(c.r), srgb_encode(c.g), srgb_encode(c.b), 1.0]
            } else {
                [c.r, c.g, c.b, 1.0]
            }
        })
    }

    /// The window's background: the app root's fill, or, where the root fills nothing (a kit
    /// root around a page), the first of the next `BACKDROP_SCAN` elements under it that covers
    /// the window with an opaque fill. It clears the drawable, so it is what shows past the
    /// frame while a Windows live resize waits for the next one (`Win::resize_live`).
    fn backdrop_fill(&self) -> Option<crate::color::Color> {
        const BACKDROP_SCAN: usize = 64;
        let root = self.app_root();
        let node = self.nodes.get(root)?;
        let fill = self.color(node.id, P_FILL, node.vis.fill);
        if fill.a > 0.0 { return Some(fill); }
        let (w, h) = self.viewport;
        // Root-space rects start at the window's origin, so either corner form covers alike.
        let covers = |r: &[f32; 4]| r[0] <= 0.5 && r[1] <= 0.5 && r[2] >= w - 0.5 && r[3] >= h - 0.5;
        let under = |mut i: usize| loop {
            match self.nodes[i].parent { Some(p) if p == root => break true, Some(p) => i = p, None => break false }
        };
        (root + 1..self.nodes.len().min(root + 1 + BACKDROP_SCAN))
            .filter(|&i| self.drawn.get(i).is_some_and(covers) && under(i))
            .map(|i| self.color(self.nodes[i].id, P_FILL, self.nodes[i].vis.fill))
            .find(|c| c.a >= 1.0)
    }
}

pub(crate) fn glyph_uv_page(uv: [f32; 2]) -> ([f32; 2], f32) {
    let page = (uv[1] as u32) / crate::text::ATLAS_SIZE;
    ([uv[0], uv[1] - (page * crate::text::ATLAS_SIZE) as f32], page as f32)
}

pub(crate) fn glyph(q: &GlyphQuad, fill: [f32; 4], clip: [f32; 4], fade: [f32; 4]) -> Instance {
    let (uv, page) = glyph_uv_page(q.uv);
    Instance {
        rect: q.rect,
        radii: [0.0; 4],
        fill,
        border: [0.0; 4],
        params: [0.0, KIND_GLYPH, page, 1.0],
        uv: [uv[0], uv[1], 0.0, 0.0],
        clip,
        fade: crate::render::pack_fade(fade, clip, clip),
    }
}

pub(crate) fn premul(c: Color, opacity: f32, srgb: bool) -> [f32; 4] {
    if srgb { c.premul_srgb(opacity) } else { c.premul(opacity) }
}

#[cfg(test)]
#[path = "blend_tests.rs"]
mod tests;
