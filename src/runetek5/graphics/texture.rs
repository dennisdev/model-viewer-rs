use std::sync::Arc;

use crate::runetek5::{io::packet::Packet, js5::Js5};

use super::sprite::SpriteData;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlphaMode {
    Opaque,
    Cutout,
    Blend,
}

pub struct MaterialInfo {
    /// If true, triangles with this material will only render the texture in high detail mode.
    /// If false and standard_detail_only is true, triangles with this material will never render.
    pub high_detail: bool,
    /// If true, triangles with this material will only be rendered in standard detail mode.
    pub standard_detail_only: bool,
    pub alpha_mode: AlphaMode,
    pub effect_id: u8,
    pub effect_config0: u8,
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self {
            high_detail: false,
            standard_detail_only: false,
            alpha_mode: AlphaMode::Opaque,
            effect_id: 0,
            effect_config0: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextureData {
    pub average_colour: u16,
    pub opaque: bool,
    pub sprite_id: u16,
    pub colour_mask: u32,
    pub anim_direction: u8,
    pub anim_speed: u8,
}

impl TextureData {
    pub fn decode(data: &[u8]) -> Self {
        let mut buf = data;

        let average_colour = buf.g2();
        let opaque = buf.g1() == 1;

        let sprite_count = buf.g1();
        if sprite_count != 1 {
            panic!("Texture sprite_count != 1");
        }

        let sprite_id = buf.g2();
        let colour_mask = buf.g4();

        let anim_direction = buf.g1();
        let anim_speed = buf.g1();

        Self {
            average_colour,
            opaque,
            sprite_id,
            colour_mask,
            anim_direction,
            anim_speed,
        }
    }
}

pub fn brighten_rgb(rgb: u32, brightness: f64) -> u32 {
    let mut r = (rgb >> 16) as f64 / 256.0;
    let mut g = (rgb >> 8 & 0xff) as f64 / 256.0;
    let mut b = (rgb & 0xff) as f64 / 256.0;
    r = f64::powf(r, brightness);
    g = f64::powf(g, brightness);
    b = f64::powf(b, brightness);
    let new_r = (r * 256.0) as u32;
    let new_g = (g * 256.0) as u32;
    let new_b = (b * 256.0) as u32;
    (new_r << 16) | (new_g << 8) | new_b
}

pub struct TextureProvider {
    pub sprite_js5: Arc<Js5>,
    pub textures: Vec<Option<TextureData>>,
}

impl TextureProvider {
    pub fn new(sprite_js5: Arc<Js5>, texture_js5: &Js5) -> Self {
        let mut textures = vec![None; texture_js5.get_file_capacity(0) as usize];
        if let Some(texture_ids) = texture_js5.get_file_ids(0) {
            for &texture_id in texture_ids.iter() {
                if let Some(data) = texture_js5.get_file(0, texture_id) {
                    let texture_data = TextureData::decode(&data);
                    textures[texture_id as usize] = Some(texture_data);
                }
            }
        }

        Self {
            sprite_js5,
            textures,
        }
    }

    pub fn get_texture_ids(&self) -> Vec<u32> {
        self.textures
            .iter()
            .enumerate()
            .filter_map(|(id, texture)| texture.as_ref().map(|_| id as u32))
            .collect()
    }

    pub fn get_loaded_percentage(&self) -> u32 {
        if self.textures.is_empty() {
            return 100;
        }
        let mut total_sprite_count = 0;
        let mut loaded_sprite_count = 0;
        for texture in self.textures.iter().flatten() {
            total_sprite_count += 1;
            if self.sprite_js5.is_ready(texture.sprite_id as u32) {
                loaded_sprite_count += 1;
            }
        }
        if total_sprite_count == 0 {
            return 100;
        }
        loaded_sprite_count * 100 / total_sprite_count
    }

    pub fn get_info(&self, id: u32) -> Option<MaterialInfo> {
        let texture_data = self.textures[id as usize].as_ref()?;
        let alpha_mode = if texture_data.opaque {
            AlphaMode::Opaque
        } else {
            AlphaMode::Blend
        };
        Some(MaterialInfo {
            standard_detail_only: false,
            high_detail: false,
            alpha_mode,
            effect_id: 0,
            effect_config0: 0,
        })
    }

    pub fn get_pixels_argb(
        &self,
        id: u32,
        width: u16,
        height: u16,
        flip_h: bool,
        brightness: f64,
    ) -> Option<Vec<u32>> {
        let texture_data = self.textures[id as usize].as_ref()?;

        let sprite_data = self.sprite_js5.get_file(texture_data.sprite_id as u32, 0)?;
        let mut pix8 = SpriteData::decode_into_pix8(&sprite_data);
        pix8.normalize();

        let pixel_count = width as usize * height as usize;
        let mut pixels = vec![0; pixel_count];

        let mut palette = Arc::unwrap_or_clone(pix8.palette);

        palette.iter_mut().for_each(|rgb| {
            let alpha = if *rgb == 0 { 0 } else { 0xff };
            *rgb = alpha << 24 | brighten_rgb(*rgb, brightness as f64);
        });

        if width == pix8.sub_width {
            pix8.pixels
                .iter()
                .enumerate()
                .for_each(|(i, &palette_index)| {
                    pixels[i] = palette[palette_index as usize];
                });
        } else if width == 128 && pix8.sub_width == 64 {
            let mut pixel_index = 0;
            for x in 0..width as usize {
                for y in 0..height as usize {
                    let src_index = ((x >> 1) << 6) + (y >> 1);
                    pixels[pixel_index] = palette[pix8.pixels[src_index] as usize];
                    pixel_index += 1;
                }
            }
        }

        Some(pixels)
    }
}
