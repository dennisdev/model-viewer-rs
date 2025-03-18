use std::sync::Arc;

use crate::runetek5::io::packet::Packet;

#[derive(Debug)]
pub struct SpriteData {
    sprite_count: u16,
    width: u16,
    height: u16,
    offsets_x: Vec<u16>,
    offsets_y: Vec<u16>,
    widths: Vec<u16>,
    heights: Vec<u16>,
    palette: Arc<Vec<u32>>,
    pixels: Vec<Vec<u8>>,
}

impl SpriteData {
    pub fn decode(data: &[u8]) -> Self {
        let mut buf = &data[data.len() - 2..];

        let sprite_count = buf.g2() as usize;

        let mut offsets_x = vec![0; sprite_count];
        let mut offsets_y = vec![0; sprite_count];
        let mut widths = vec![0; sprite_count];
        let mut heights = vec![0; sprite_count];
        let mut sprite_pixels: Vec<Vec<u8>> = Vec::with_capacity(sprite_count);

        buf = &data[data.len() - 7 - sprite_count * 8..];

        let width = buf.g2();
        let height = buf.g2();
        let palette_size = (buf.g1() as usize) + 1;

        for i in 0..sprite_count {
            offsets_x[i] = buf.g2();
        }
        for i in 0..sprite_count {
            offsets_y[i] = buf.g2();
        }
        for i in 0..sprite_count {
            widths[i] = buf.g2();
        }
        for i in 0..sprite_count {
            heights[i] = buf.g2();
        }

        buf = &data[data.len() - 7 - sprite_count * 8 - (palette_size - 1) * 3..];

        let mut palette = vec![0; palette_size];
        for i in 1..palette_size {
            palette[i] = buf.g3();
            if palette[i] == 0 {
                palette[i] = 1;
            }
        }

        buf = &data;

        for i in 0..sprite_count {
            let width = widths[i] as usize;
            let height = heights[i] as usize;
            let pixel_count = width * height;
            let mut pixels = vec![0; pixel_count];
            let pixel_order = buf.g1();
            if pixel_order == 0 {
                // row first
                for j in 0..pixel_count {
                    pixels[j] = buf.g1();
                }
            } else if pixel_order == 1 {
                // column first
                for x in 0..width {
                    for y in 0..height {
                        pixels[x + y * width] = buf.g1();
                    }
                }
            }
            sprite_pixels.push(pixels);
        }

        Self {
            sprite_count: sprite_count as u16,
            width,
            height,
            offsets_x,
            offsets_y,
            widths,
            heights,
            palette: Arc::new(palette),
            pixels: sprite_pixels,
        }
    }

    pub fn decode_into_pix8s(data: &[u8]) -> Vec<Pix8> {
        let sprite_data = SpriteData::decode(data);

        sprite_data
            .pixels
            .into_iter()
            .zip(sprite_data.offsets_x.into_iter().zip(sprite_data.offsets_y))
            .zip(sprite_data.widths.into_iter().zip(sprite_data.heights))
            .map(|((pixels, (offset_x, offset_y)), (width, height))| {
                Pix8::from_data(
                    sprite_data.width,
                    sprite_data.height,
                    offset_x,
                    offset_y,
                    width,
                    height,
                    sprite_data.palette.clone(),
                    pixels,
                )
            })
            .collect()
    }

    pub fn decode_into_pix8(data: &[u8]) -> Pix8 {
        let mut sprite_data = SpriteData::decode(data);

        Pix8::from_data(
            sprite_data.width,
            sprite_data.height,
            sprite_data.offsets_x[0],
            sprite_data.offsets_y[0],
            sprite_data.widths[0],
            sprite_data.heights[0],
            sprite_data.palette.clone(),
            std::mem::take(&mut sprite_data.pixels[0]),
        )
    }
}

pub struct Pix8 {
    pub width: u16,
    pub height: u16,
    pub offset_x: u16,
    pub offset_y: u16,
    pub sub_width: u16,
    pub sub_height: u16,
    pub palette: Arc<Vec<u32>>,
    pub pixels: Vec<u8>,
}

impl Pix8 {
    pub fn from_data(
        width: u16,
        height: u16,
        offset_x: u16,
        offset_y: u16,
        sub_width: u16,
        sub_height: u16,
        palette: Arc<Vec<u32>>,
        pixels: Vec<u8>,
    ) -> Self {
        Self {
            width,
            height,
            offset_x,
            offset_y,
            sub_width,
            sub_height,
            palette,
            pixels,
        }
    }

    pub fn normalize(&mut self) {
        if self.width == self.sub_width && self.height == self.sub_height {
            return;
        }
        let width = self.width as usize;
        let height = self.height as usize;
        let offset_x = self.offset_x as usize;
        let offset_y = self.offset_y as usize;
        let mut pixels = vec![0; width * height];
        let mut i = 0;
        for y in 0..self.sub_height as usize {
            for x in 0..self.sub_width as usize {
                pixels[(x + offset_x) + (y + offset_y) * width] = self.pixels[i];
                i += 1;
            }
        }
        self.pixels = pixels;
        self.offset_x = 0;
        self.offset_y = 0;
        self.sub_width = self.width;
        self.sub_height = self.height;
    }
}
