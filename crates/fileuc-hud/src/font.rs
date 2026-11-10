use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use tiny_skia::{Color, Pixmap, PremultipliedColorU8};

pub struct FontContext {
    bold: Option<FontArc>,
    regular: Option<FontArc>,
}

impl Default for FontContext {
    fn default() -> Self {
        Self::new()
    }
}

impl FontContext {
    pub fn new() -> Self {
        let bold = std::fs::read(r"C:\Windows\Fonts\segoeuib.ttf")
            .ok()
            .and_then(|bytes| FontArc::try_from_vec(bytes).ok());
        let regular = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf")
            .ok()
            .and_then(|bytes| FontArc::try_from_vec(bytes).ok());

        Self { bold, regular }
    }

    pub fn draw_text_centered(
        &self,
        pixmap: &mut Pixmap,
        text: &str,
        cx: f32,
        cy: f32,
        font_size: f32,
        color: Color,
        bold: bool,
    ) {
        let font_opt = if bold {
            self.bold.as_ref().or(self.regular.as_ref())
        } else {
            self.regular.as_ref().or(self.bold.as_ref())
        };

        let font = match font_opt {
            Some(f) => f,
            None => return,
        };

        let scale = PxScale::from(font_size);
        let scaled_font = font.as_scaled(scale);

        let mut glyph_data = Vec::new();
        let mut current_x = 0.0f32;
        let mut last_glyph: Option<ab_glyph::GlyphId> = None;

        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            let glyph_id = scaled_font.glyph_id(ch);
            if let Some(prev) = last_glyph {
                current_x += scaled_font.kern(prev, glyph_id);
            }
            let rel_x = current_x;
            current_x += scaled_font.h_advance(glyph_id);
            last_glyph = Some(glyph_id);
            glyph_data.push((ch, rel_x));
        }

        let total_width = current_x;
        let ascent = scaled_font.ascent();
        let cap_height = ascent * 0.72;

        let start_x = cx - total_width / 2.0;
        let baseline_y = cy + (cap_height / 2.0);

        let width = pixmap.width() as i32;
        let height = pixmap.height() as i32;
        let pixels = pixmap.pixels_mut();

        for (ch, rel_x) in glyph_data {
            let mut glyph = scaled_font.scaled_glyph(ch);
            glyph.position = ab_glyph::point(start_x + rel_x, baseline_y);

            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    if coverage <= 0.01 {
                        return;
                    }
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;

                    if px >= 0 && px < width && py >= 0 && py < height {
                        let idx = (py * width + px) as usize;
                        let dst = pixels[idx];

                        let a_factor = color.alpha() * coverage;
                        let src_a = (a_factor * 255.0).round() as u8;
                        if src_a == 0 {
                            return;
                        }

                        let src_r = ((color.red() * color.alpha() * coverage) * 255.0).round() as u8;
                        let src_g = ((color.green() * color.alpha() * coverage) * 255.0).round() as u8;
                        let src_b = ((color.blue() * color.alpha() * coverage) * 255.0).round() as u8;

                        let inv_a = 255 - src_a as u32;
                        let out_r = src_r as u32 + (dst.red() as u32 * inv_a + 127) / 255;
                        let out_g = src_g as u32 + (dst.green() as u32 * inv_a + 127) / 255;
                        let out_b = src_b as u32 + (dst.blue() as u32 * inv_a + 127) / 255;
                        let out_a = src_a as u32 + (dst.alpha() as u32 * inv_a + 127) / 255;

                        if let Some(c) = PremultipliedColorU8::from_rgba(
                            out_r.min(255) as u8,
                            out_g.min(255) as u8,
                            out_b.min(255) as u8,
                            out_a.min(255) as u8,
                        ) {
                            pixels[idx] = c;
                        }
                    }
                });
            }
        }
    }
}
