use std::sync::Arc;

use bitflags::bitflags;

use crate::runetek5::{
    io::packet::Packet,
    js5::Js5,
    math::trig::{JagDegrees, COSINE, SINE},
};

use super::texture::{AlphaMode, TextureProvider};

pub type Hsl = u16;
pub type Rgb = u32;

pub struct ModelTextureMappingProps {
    render_types: Vec<u8>,
    mapping_p: Vec<u16>,
    mapping_m: Vec<u16>,
    mapping_n: Vec<u16>,
}

impl ModelTextureMappingProps {
    fn new(textured_triangle_count: usize) -> Self {
        Self {
            render_types: vec![0; textured_triangle_count],
            mapping_p: vec![0; textured_triangle_count],
            mapping_m: vec![0; textured_triangle_count],
            mapping_n: vec![0; textured_triangle_count],
        }
    }
}

pub struct ModelComplexTextureMappingProps {
    scale_x: Vec<i32>,
    scale_y: Vec<i32>,
    scale_z: Vec<i32>,
    rotation: Vec<i8>,
    direction: Vec<i8>,
    speed: Vec<i8>,
}

pub struct ModelAnimMayaProps {
    groups: Vec<Vec<u8>>,
    scales: Vec<Vec<u8>>,
}

impl ModelAnimMayaProps {
    fn new(vertex_count: usize) -> Self {
        Self {
            groups: Vec::with_capacity(vertex_count),
            scales: Vec::with_capacity(vertex_count),
        }
    }
}

struct ModelMergeVertices {
    vertex_count: u16,
    vertex_x: Vec<i32>,
    vertex_y: Vec<i32>,
    vertex_z: Vec<i32>,
    vertex_model_index_flags: Vec<u16>,
    vertex_skins: Vec<i32>,
}

struct ModelMergeMaterialTriangles {
    render_types: Vec<u8>,
    // Simple
    mapping_p: Vec<u16>,
    mapping_m: Vec<u16>,
    mapping_n: Vec<u16>,

    // Complex
    scale_x: Vec<i32>,
    scale_y: Vec<i32>,
    scale_z: Vec<i32>,
    rotation: Vec<i8>,
    direction: Vec<i8>,
    speed: Vec<i8>,
}

pub struct ModelUnlit {
    pub version: u8,
    pub vertex_count: u16,
    pub triangle_count: u16,
    pub textured_triangle_count: u16,
    pub priority: u8,
    pub used_vertex_count: u16,
    pub vertex_x: Arc<Vec<i32>>,
    pub vertex_y: Arc<Vec<i32>>,
    pub vertex_z: Arc<Vec<i32>>,
    pub triangle_a: Vec<u16>,
    pub triangle_b: Vec<u16>,
    pub triangle_c: Vec<u16>,
    pub triangle_render_type: Option<Vec<u8>>,
    pub triangle_colour: Vec<Hsl>,
    pub triangle_transparency: Option<Vec<u8>>,
    pub triangle_material: Option<Vec<i16>>,
    pub triangle_texture_coords: Option<Vec<i16>>,
    pub triangle_priority: Option<Vec<u8>>,
    pub texture_props: Option<ModelTextureMappingProps>,
    pub texture_complex_props: Option<ModelComplexTextureMappingProps>,
    pub vertex_skins: Option<Vec<i32>>,
    pub triangle_skins: Option<Vec<i32>>,
    pub anim_maya_props: Option<ModelAnimMayaProps>,
}

impl ModelUnlit {
    const VERSION: u8 = 12;

    pub fn new() -> Self {
        Self {
            version: Self::VERSION,
            vertex_count: 0,
            triangle_count: 0,
            textured_triangle_count: 0,
            priority: 0,
            used_vertex_count: 0,
            vertex_x: Arc::new(Vec::new()),
            vertex_y: Arc::new(Vec::new()),
            vertex_z: Arc::new(Vec::new()),
            triangle_a: Vec::new(),
            triangle_b: Vec::new(),
            triangle_c: Vec::new(),
            triangle_render_type: None,
            triangle_colour: Vec::new(),
            triangle_transparency: None,
            triangle_material: None,
            triangle_texture_coords: None,
            triangle_priority: None,
            texture_props: None,
            texture_complex_props: None,
            vertex_skins: None,
            triangle_skins: None,
            anim_maya_props: None,
        }
    }

    pub fn merge(models: &[ModelUnlit]) -> Self {
        let mut vertex_count = 0u16;
        let mut triangle_count = 0u16;
        let mut textured_triangle_count = 0u16;

        let mut priority: Option<u8> = None;

        let mut has_priority = false;
        let mut has_render_type = false;
        let mut has_transparency = false;
        let mut has_material = false;
        let mut has_texture_coord = false;
        let mut has_triangle_skin = false;
        let mut has_maya_group = false;

        for model in models {
            vertex_count += model.vertex_count;
            triangle_count += model.triangle_count;
            textured_triangle_count += model.textured_triangle_count;

            if *priority.get_or_insert(model.priority) != model.priority {
                has_priority = true;
            }

            has_render_type |= model.triangle_render_type.is_some();
            has_transparency |= model.triangle_transparency.is_some();
            has_material |= model.triangle_material.is_some();
            has_texture_coord |= model.triangle_texture_coords.is_some();
            has_triangle_skin |= model.triangle_skins.is_some();
            has_maya_group |= model.anim_maya_props.is_some();
        }

        let mut vertices = ModelMergeVertices {
            vertex_count: 0,
            vertex_x: vec![0; vertex_count as usize],
            vertex_y: vec![0; vertex_count as usize],
            vertex_z: vec![0; vertex_count as usize],
            vertex_model_index_flags: vec![0; vertex_count as usize],
            vertex_skins: vec![0; vertex_count as usize],
        };

        let mut triangle_a = vec![0u16; triangle_count as usize];
        let mut triangle_b = vec![0u16; triangle_count as usize];
        let mut triangle_c = vec![0u16; triangle_count as usize];

        let mut triangle_model_index_flags = vec![0u16; triangle_count as usize];
        let mut triangle_colour = vec![0u16; triangle_count as usize];
        let mut triangle_priority = if has_priority {
            Some(vec![0u8; triangle_count as usize])
        } else {
            None
        };
        let mut triangle_render_type = if has_render_type {
            Some(vec![0u8; triangle_count as usize])
        } else {
            None
        };
        let mut triangle_transparency = if has_transparency {
            Some(vec![0u8; triangle_count as usize])
        } else {
            None
        };
        let mut triangle_material = if has_material {
            Some(vec![-1i16; triangle_count as usize])
        } else {
            None
        };
        let mut triangle_texture_coords = if has_texture_coord {
            Some(vec![-1i16; triangle_count as usize])
        } else {
            None
        };
        let mut triangle_skins = if has_triangle_skin {
            Some(vec![-1i32; triangle_count as usize])
        } else {
            None
        };

        triangle_count = 0;
        for (index, model) in models.iter().enumerate() {
            let index_flag = 1 << index;
            let start_triangle_count = triangle_count as usize;
            Self::copy_priorities(start_triangle_count, triangle_priority.as_mut(), model);
            Self::copy_render_types(start_triangle_count, triangle_render_type.as_mut(), model);
            Self::copy_transparencies(start_triangle_count, triangle_transparency.as_mut(), model);
            Self::copy_materials(start_triangle_count, triangle_material.as_mut(), model);
            Self::copy_triangle_skins(start_triangle_count, triangle_skins.as_mut(), model);
            for t in 0..model.triangle_count as usize {
                let new_t = triangle_count as usize;

                triangle_a[new_t] =
                    Self::copy_vertex(&mut vertices, model, model.triangle_a[t], index_flag);
                triangle_b[new_t] =
                    Self::copy_vertex(&mut vertices, model, model.triangle_b[t], index_flag);
                triangle_c[new_t] =
                    Self::copy_vertex(&mut vertices, model, model.triangle_c[t], index_flag);
                triangle_model_index_flags[new_t] = index_flag;
                triangle_colour[new_t] = model.triangle_colour[t];

                triangle_count += 1;
            }
        }

        let used_vertex_count = vertices.vertex_count;

        let mut mat_triangles = if textured_triangle_count > 0 {
            let textured_triangle_count = textured_triangle_count as usize;
            Some(ModelMergeMaterialTriangles {
                render_types: vec![0; textured_triangle_count],
                mapping_p: vec![0; textured_triangle_count],
                mapping_m: vec![0; textured_triangle_count],
                mapping_n: vec![0; textured_triangle_count],
                scale_x: vec![0; textured_triangle_count],
                scale_y: vec![0; textured_triangle_count],
                scale_z: vec![0; textured_triangle_count],
                rotation: vec![0; textured_triangle_count],
                direction: vec![0; textured_triangle_count],
                speed: vec![0; textured_triangle_count],
            })
        } else {
            None
        };

        let mut tex_coord_count = 0usize;
        textured_triangle_count = 0;

        for (index, model) in models.iter().enumerate() {
            let index_flag = 1 << index;
            Self::copy_texture_coords(
                textured_triangle_count,
                &mut tex_coord_count,
                triangle_texture_coords.as_mut(),
                model,
            );
            let Some(mat_triangles) = mat_triangles.as_mut() else {
                continue;
            };
            let Some(src_props) = model.texture_props.as_ref() else {
                continue;
            };
            for t in 0..model.textured_triangle_count as usize {
                let new_t = textured_triangle_count as usize;
                let mapping_type = src_props.render_types[t];
                mat_triangles.render_types[new_t] = mapping_type;
                if mapping_type == 0 {
                    mat_triangles.mapping_p[new_t] =
                        Self::copy_vertex(&mut vertices, model, src_props.mapping_p[t], index_flag);
                    mat_triangles.mapping_m[new_t] =
                        Self::copy_vertex(&mut vertices, model, src_props.mapping_m[t], index_flag);
                    mat_triangles.mapping_n[new_t] =
                        Self::copy_vertex(&mut vertices, model, src_props.mapping_n[t], index_flag);
                } else if mapping_type == 1 {
                    mat_triangles.mapping_p[new_t] = src_props.mapping_p[t];
                    mat_triangles.mapping_m[new_t] = src_props.mapping_m[t];
                    mat_triangles.mapping_n[new_t] = src_props.mapping_n[t];
                }

                textured_triangle_count += 1;
            }
        }

        vertex_count = vertices.vertex_count;

        let texture_props = mat_triangles.map(|triangles| ModelTextureMappingProps {
            render_types: triangles.render_types,
            mapping_p: triangles.mapping_p,
            mapping_m: triangles.mapping_m,
            mapping_n: triangles.mapping_n,
        });

        Self {
            version: Self::VERSION,
            vertex_count,
            used_vertex_count,
            triangle_count,
            textured_triangle_count,
            priority: priority.unwrap_or(0),
            vertex_x: Arc::new(vertices.vertex_x),
            vertex_y: Arc::new(vertices.vertex_y),
            vertex_z: Arc::new(vertices.vertex_z),
            triangle_a,
            triangle_b,
            triangle_c,
            triangle_render_type,
            triangle_colour,
            triangle_transparency,
            triangle_material,
            triangle_texture_coords,
            triangle_priority,
            texture_props,
            texture_complex_props: None,
            vertex_skins: Some(vertices.vertex_skins),
            triangle_skins,
            anim_maya_props: None,
        }
    }

    fn copy_priorities(
        start_triangle_count: usize,
        dst_priority: Option<&mut Vec<u8>>,
        model: &ModelUnlit,
    ) {
        let Some(dst_priority) = dst_priority else {
            return;
        };
        if let Some(src_priority) = model.triangle_priority.as_ref() {
            for t in 0..model.triangle_count as usize {
                let new_t = start_triangle_count + t;
                dst_priority[new_t] = src_priority[t];
            }
        } else {
            for t in 0..model.triangle_count as usize {
                let new_t = start_triangle_count + t;
                dst_priority[new_t] = model.priority;
            }
        }
    }

    fn copy_render_types(
        start_triangle_count: usize,
        dst_render_type: Option<&mut Vec<u8>>,
        model: &ModelUnlit,
    ) {
        let Some(dst_render_type) = dst_render_type else {
            return;
        };
        let Some(src_render_type) = model.triangle_render_type.as_ref() else {
            return;
        };
        for t in 0..model.triangle_count as usize {
            let new_t = start_triangle_count + t;
            dst_render_type[new_t] = src_render_type[t];
        }
    }

    fn copy_transparencies(
        start_triangle_count: usize,
        dst_transparency: Option<&mut Vec<u8>>,
        model: &ModelUnlit,
    ) {
        let Some(dst_transparency) = dst_transparency else {
            return;
        };
        let Some(src_transparency) = model.triangle_transparency.as_ref() else {
            return;
        };
        for t in 0..model.triangle_count as usize {
            let new_t = start_triangle_count + t;
            dst_transparency[new_t] = src_transparency[t];
        }
    }

    fn copy_materials(
        start_triangle_count: usize,
        dst_material: Option<&mut Vec<i16>>,
        model: &ModelUnlit,
    ) {
        let Some(dst_material) = dst_material else {
            return;
        };
        let Some(src_material) = model.triangle_material.as_ref() else {
            return;
        };
        for t in 0..model.triangle_count as usize {
            let new_t = start_triangle_count + t;
            dst_material[new_t] = src_material[t];
        }
    }

    fn copy_triangle_skins(
        start_triangle_count: usize,
        dst_skins: Option<&mut Vec<i32>>,
        model: &ModelUnlit,
    ) {
        let Some(dst_skins) = dst_skins else {
            return;
        };
        let Some(src_skins) = model.triangle_skins.as_ref() else {
            return;
        };
        for t in 0..model.triangle_count as usize {
            let new_t = start_triangle_count + t;
            dst_skins[new_t] = src_skins[t];
        }
    }

    fn copy_texture_coords(
        textured_triangle_count: u16,
        tex_coord_count: &mut usize,
        dst_coords: Option<&mut Vec<i16>>,
        model: &ModelUnlit,
    ) {
        let Some(dst_coords) = dst_coords else {
            return;
        };
        let Some(src_coords) = model.triangle_texture_coords.as_ref() else {
            return;
        };
        for t in 0..model.triangle_count as usize {
            let coord = src_coords[t];
            if coord >= 0 && coord < 32766 {
                dst_coords[*tex_coord_count] = textured_triangle_count as i16 + coord;
            } else {
                dst_coords[*tex_coord_count] = coord;
            }

            *tex_coord_count += 1;
        }
    }

    fn copy_vertex(
        vertices: &mut ModelMergeVertices,
        model: &ModelUnlit,
        src_index: u16,
        model_index_flag: u16,
    ) -> u16 {
        let src_index = src_index as usize;
        let x = model.vertex_x[src_index];
        let y = model.vertex_y[src_index];
        let z = model.vertex_z[src_index];
        for i in 0..vertices.vertex_count as usize {
            if vertices.vertex_x[i] == x && vertices.vertex_y[i] == y && vertices.vertex_z[i] == z {
                vertices.vertex_model_index_flags[i] |= model_index_flag;
                return i as u16;
            }
        }

        let dst_index = vertices.vertex_count as usize;
        vertices.vertex_x[dst_index] = x;
        vertices.vertex_y[dst_index] = y;
        vertices.vertex_z[dst_index] = z;
        vertices.vertex_model_index_flags[dst_index] = model_index_flag;
        vertices.vertex_skins[dst_index] = model
            .vertex_skins
            .as_ref()
            .map_or(-1, |skins| skins[src_index]);

        vertices.vertex_count += 1;

        dst_index as u16
    }

    pub fn from_js5(js5: &Js5, group_id: u32, file_id: u32) -> Option<Self> {
        let data = js5.get_file(group_id, file_id)?;
        Some(Self::from_data(&data))
    }

    pub fn from_data(data: &[u8]) -> Self {
        let mut model = Self::new();
        model.decode(data);
        model
    }

    pub fn decode(&mut self, data: &[u8]) {
        let mut version_buf = &data[data.len() - 2..];
        let version = 65536 - version_buf.g2() as u32;
        match version {
            3 => {
                self.decode_v1_maya(data);
            }
            2 => {
                self.decode_v0_maya(data);
            }
            1 => {
                self.decode_v1(data);
            }
            _ => {
                self.decode_v0(data);
            }
        }
    }

    fn decode_v0(&mut self, data: &[u8]) {
        // println!("v0");
        let mut buf1 = data;
        let mut buf2 = data;
        let mut buf3 = data;
        let mut buf4 = data;
        let mut buf5 = data;
        buf1 = &data[(data.len() - 18)..];
        let vertex_count = buf1.g2() as usize;
        let triangle_count = buf1.g2() as usize;
        let textured_triangle_count = buf1.g1() as usize;
        let has_textures = buf1.g1() == 1;
        let priority = buf1.g1();
        let has_priorities = priority == 255;
        let has_transparencies = buf1.g1() == 1;
        let has_triangle_skins = buf1.g1() == 1;
        let has_vertex_skins = buf1.g1() == 1;
        let vertex_x_count = buf1.g2() as usize;
        let vertex_y_count = buf1.g2() as usize;
        let vertex_z_count = buf1.g2() as usize;
        let index_count = buf1.g2() as usize;
        let vertex_flags_offset = 0;
        let mut offset = vertex_flags_offset + vertex_count;
        let index_types_offset = offset;
        offset += triangle_count;
        let priorities_offset = offset;
        if has_priorities {
            offset += triangle_count;
        }
        let triangle_skins_offset = offset;
        if has_triangle_skins {
            offset += triangle_count;
        }
        let texture_flags_offset = offset;
        if has_textures {
            offset += triangle_count;
        }
        let vertex_skins_offset = offset;
        if has_vertex_skins {
            offset += vertex_count;
        }
        let transparencies_offset = offset;
        if has_transparencies {
            offset += triangle_count;
        }
        let indices_offset = offset;
        offset += index_count;
        let colours_offset = offset;
        offset += triangle_count * 2;
        let texture_mapping_offset = offset;
        offset += textured_triangle_count * 6;
        let vertex_x_offset = offset;
        offset += vertex_x_count;
        let vertex_y_offset = offset;
        offset += vertex_y_count;
        let vertex_z_offset = offset;
        offset += vertex_z_count;

        self.vertex_count = vertex_count as u16;
        self.triangle_count = triangle_count as u16;
        self.textured_triangle_count = textured_triangle_count as u16;
        self.vertex_x = Arc::new(vec![0; vertex_count]);
        self.vertex_y = Arc::new(vec![0; vertex_count]);
        self.vertex_z = Arc::new(vec![0; vertex_count]);
        self.triangle_a = vec![0; triangle_count];
        self.triangle_b = vec![0; triangle_count];
        self.triangle_c = vec![0; triangle_count];

        if textured_triangle_count > 0 {
            self.texture_props = Some(ModelTextureMappingProps::new(textured_triangle_count));
            // self.texture_render_types = Some(vec![0; textured_triangle_count]);
            // self.texture_mapping_p = Some(vec![0; textured_triangle_count]);
            // self.texture_mapping_m = Some(vec![0; textured_triangle_count]);
            // self.texture_mapping_n = Some(vec![0; textured_triangle_count]);
        }

        if has_vertex_skins {
            self.vertex_skins = Some(vec![0; vertex_count]);
        }

        if has_textures {
            self.triangle_render_type = Some(vec![0; triangle_count]);
            self.triangle_material = Some(vec![0; triangle_count]);
            self.triangle_texture_coords = Some(vec![0; triangle_count]);
        }

        if has_priorities {
            self.triangle_priority = Some(vec![0; triangle_count]);
        } else {
            self.priority = priority;
        }

        if has_transparencies {
            self.triangle_transparency = Some(vec![0; triangle_count]);
        }

        if has_triangle_skins {
            self.triangle_skins = Some(vec![0; triangle_count]);
        }

        self.triangle_colour = vec![0; triangle_count];

        buf1 = &data[vertex_flags_offset..];
        buf2 = &data[vertex_x_offset..];
        buf3 = &data[vertex_y_offset..];
        buf4 = &data[vertex_z_offset..];
        buf5 = &data[vertex_skins_offset..];

        self.decode_vertices(
            vertex_count,
            has_vertex_skins,
            false,
            false,
            &mut buf1,
            &mut buf2,
            &mut buf3,
            &mut buf4,
            &mut buf5,
        );

        buf1 = &data[colours_offset..];
        buf2 = &data[texture_flags_offset..];
        buf3 = &data[priorities_offset..];
        buf4 = &data[transparencies_offset..];
        buf5 = &data[triangle_skins_offset..];

        self.decode_triangles(
            triangle_count,
            has_textures,
            has_priorities,
            has_transparencies,
            has_triangle_skins,
            &mut buf1,
            &mut buf2,
            &mut buf3,
            &mut buf4,
            &mut buf5,
        );

        buf1 = &data[indices_offset..];
        buf2 = &data[index_types_offset..];

        self.decode_indices(triangle_count, &mut buf1, &mut buf2);

        buf1 = &data[texture_mapping_offset..];

        self.decode_texture_mapping(textured_triangle_count, &mut buf1);
    }

    fn decode_v1(&mut self, data: &[u8]) {
        // println!("v1");
        let mut buf1 = data;
        let buf2 = data;
        let buf3 = data;
        let buf4 = data;
        let buf5 = data;
        let buf6 = data;
        let buf7 = data;
        buf1.skip(data.len() - 23);
    }

    fn decode_v0_maya(&mut self, data: &[u8]) {
        // println!("v2");
        let mut buf1 = data;
        let mut buf2 = data;
        let mut buf3 = data;
        let mut buf4 = data;
        let mut buf5 = data;
        buf1 = &data[(data.len() - 23)..];
        let vertex_count = buf1.g2() as usize;
        let triangle_count = buf1.g2() as usize;
        let textured_triangle_count = buf1.g1() as usize;
        let has_textures = buf1.g1() == 1;
        let priority = buf1.g1();
        let has_priorities = priority == 255;
        let has_transparencies = buf1.g1() == 1;
        let has_triangle_skins = buf1.g1() == 1;
        let has_vertex_skins = buf1.g1() == 1;
        let has_maya_groups = buf1.g1() == 1;
        let vertex_x_count = buf1.g2() as usize;
        let vertex_y_count = buf1.g2() as usize;
        let vertex_z_count = buf1.g2() as usize;
        let index_count = buf1.g2() as usize;
        let vertex_skins_size = buf1.g2() as usize;
        let vertex_flags_offset = 0;
        let mut offset = vertex_flags_offset + vertex_count;
        let index_types_offset = offset;
        offset += triangle_count;
        let priorities_offset = offset;
        if has_priorities {
            offset += triangle_count;
        }
        let triangle_skins_offset = offset;
        if has_triangle_skins {
            offset += triangle_count;
        }
        let texture_flags_offset = offset;
        if has_textures {
            offset += triangle_count;
        }
        let vertex_skins_offset = offset;
        offset += vertex_skins_size;
        let transparencies_offset = offset;
        if has_transparencies {
            offset += triangle_count;
        }
        let indices_offset = offset;
        offset += index_count;
        let colours_offset = offset;
        offset += triangle_count * 2;
        let texture_mapping_offset = offset;
        offset += textured_triangle_count * 6;
        let vertex_x_offset = offset;
        offset += vertex_x_count;
        let vertex_y_offset = offset;
        offset += vertex_y_count;
        let vertex_z_offset = offset;
        offset += vertex_z_count;

        self.vertex_count = vertex_count as u16;
        self.triangle_count = triangle_count as u16;
        self.textured_triangle_count = textured_triangle_count as u16;
        self.vertex_x = Arc::new(vec![0; vertex_count]);
        self.vertex_y = Arc::new(vec![0; vertex_count]);
        self.vertex_z = Arc::new(vec![0; vertex_count]);
        self.triangle_a = vec![0; triangle_count];
        self.triangle_b = vec![0; triangle_count];
        self.triangle_c = vec![0; triangle_count];

        self.triangle_colour = vec![0; triangle_count];

        if textured_triangle_count > 0 {
            self.texture_props = Some(ModelTextureMappingProps::new(textured_triangle_count));
            // self.texture_render_types = Some(vec![0; textured_triangle_count]);
            // self.texture_mapping_p = Some(vec![0; textured_triangle_count]);
            // self.texture_mapping_m = Some(vec![0; textured_triangle_count]);
            // self.texture_mapping_n = Some(vec![0; textured_triangle_count]);
        }
        if has_vertex_skins {
            self.vertex_skins = Some(vec![0; vertex_count]);
        }
        if has_textures {
            self.triangle_render_type = Some(vec![0; triangle_count]);
            self.triangle_material = Some(vec![0; triangle_count]);
            self.triangle_texture_coords = Some(vec![0; triangle_count]);
        }
        if has_priorities {
            self.triangle_priority = Some(vec![0; triangle_count]);
        } else {
            self.priority = priority;
        }
        if has_transparencies {
            self.triangle_transparency = Some(vec![0; triangle_count]);
        }
        if has_triangle_skins {
            self.triangle_skins = Some(vec![0; triangle_count]);
        }
        if has_maya_groups {
            self.anim_maya_props = Some(ModelAnimMayaProps::new(vertex_count));
        }

        buf1 = &data[vertex_flags_offset..];
        buf2 = &data[vertex_x_offset..];
        buf3 = &data[vertex_y_offset..];
        buf4 = &data[vertex_z_offset..];
        buf5 = &data[vertex_skins_offset..];

        self.decode_vertices(
            vertex_count,
            has_vertex_skins,
            false,
            has_maya_groups,
            &mut buf1,
            &mut buf2,
            &mut buf3,
            &mut buf4,
            &mut buf5,
        );

        buf1 = &data[colours_offset..];
        buf2 = &data[texture_flags_offset..];
        buf3 = &data[priorities_offset..];
        buf4 = &data[transparencies_offset..];
        buf5 = &data[triangle_skins_offset..];

        self.decode_triangles(
            triangle_count,
            has_textures,
            has_priorities,
            has_transparencies,
            has_triangle_skins,
            &mut buf1,
            &mut buf2,
            &mut buf3,
            &mut buf4,
            &mut buf5,
        );

        buf1 = &data[indices_offset..];
        buf2 = &data[index_types_offset..];

        self.decode_indices(triangle_count, &mut buf1, &mut buf2);

        buf1 = &data[texture_mapping_offset..];

        self.decode_texture_mapping(textured_triangle_count, &mut buf1);
    }

    fn decode_vertices(
        &mut self,
        vertex_count: usize,
        has_vertex_skins: bool,
        has_extended_vertex_skins: bool,
        has_maya_groups: bool,
        vertex_flags_buf: &mut &[u8],
        vertex_x_buf: &mut &[u8],
        vertex_y_buf: &mut &[u8],
        vertex_z_buf: &mut &[u8],
        vertex_skins_buf: &mut &[u8],
    ) {
        let vertex_x = Arc::get_mut(&mut self.vertex_x).unwrap();
        let vertex_y = Arc::get_mut(&mut self.vertex_y).unwrap();
        let vertex_z = Arc::get_mut(&mut self.vertex_z).unwrap();

        let mut last_x = 0;
        let mut last_y = 0;
        let mut last_z = 0;
        for i in 0..vertex_count {
            let flags = vertex_flags_buf.g1();
            let delta_x = if flags & 0x1 != 0 {
                vertex_x_buf.get_smart_1_or_2s()
            } else {
                0
            };
            let delta_y = if flags & 0x2 != 0 {
                vertex_y_buf.get_smart_1_or_2s()
            } else {
                0
            };
            let delta_z = if flags & 0x4 != 0 {
                vertex_z_buf.get_smart_1_or_2s()
            } else {
                0
            };
            vertex_x[i] = last_x + delta_x;
            vertex_y[i] = last_y + delta_y;
            vertex_z[i] = last_z + delta_z;
            last_x = vertex_x[i];
            last_y = vertex_y[i];
            last_z = vertex_z[i];
        }

        if has_vertex_skins {
            let vertex_skins = self.vertex_skins.as_mut().unwrap();
            for i in 0..vertex_count {
                let v = if has_extended_vertex_skins {
                    vertex_skins_buf.get_smart_1_or_2_null()
                } else {
                    match vertex_skins_buf.g1() {
                        255 => -1,
                        n => n as i32,
                    }
                };
                vertex_skins[i] = v;
            }
        }

        if has_maya_groups {
            let anim_maya_props = self.anim_maya_props.as_mut().unwrap();
            for _ in 0..vertex_count {
                let count = vertex_skins_buf.g1() as usize;

                let mut maya_groups = vec![0; count as usize];
                let mut maya_scales = vec![0; count as usize];

                for i in 0..count {
                    maya_groups[i] = vertex_skins_buf.g1();
                    maya_scales[i] = vertex_skins_buf.g1();
                }

                anim_maya_props.groups.push(maya_groups);
                anim_maya_props.scales.push(maya_scales);
            }
        }
    }

    fn decode_triangles(
        &mut self,
        triangle_count: usize,
        has_textures: bool,
        has_priorities: bool,
        has_transparencies: bool,
        has_triangle_skins: bool,
        colour_buf: &mut &[u8],
        texture_flag_buf: &mut &[u8],
        priority_buf: &mut &[u8],
        transparency_buf: &mut &[u8],
        triangle_skin_buf: &mut &[u8],
    ) {
        for i in 0..triangle_count {
            self.triangle_colour[i] = colour_buf.g2();
        }
        if has_textures {
            let triangle_render_types = self.triangle_render_type.as_mut().unwrap();
            let triangle_textures = self.triangle_material.as_mut().unwrap();
            let triangle_texture_coords = self.triangle_texture_coords.as_mut().unwrap();
            for i in 0..triangle_count {
                let flags = texture_flag_buf.g1();
                if flags & 0x1 != 0 {
                    triangle_render_types[i] = 1;
                } else {
                    triangle_render_types[i] = 0;
                }
                if flags & 0x2 != 0 {
                    let texture_id = self.triangle_colour[i] as i16;
                    triangle_textures[i] = texture_id;
                    triangle_texture_coords[i] = (flags >> 2) as i16;
                    self.triangle_colour[i] = 127;
                } else {
                    triangle_textures[i] = -1;
                    triangle_texture_coords[i] = -1;
                }
            }
        }
        if has_priorities {
            let triangle_priorities = self.triangle_priority.as_mut().unwrap();
            for i in 0..triangle_count {
                triangle_priorities[i] = priority_buf.g1();
            }
        }
        if has_transparencies {
            let triangle_transparencies = self.triangle_transparency.as_mut().unwrap();
            for i in 0..triangle_count {
                triangle_transparencies[i] = transparency_buf.g1();
            }
        }
        if has_triangle_skins {
            let triangle_skins = self.triangle_skins.as_mut().unwrap();
            for i in 0..triangle_count {
                triangle_skins[i] = triangle_skin_buf.g1() as i32;
            }
        }
    }

    fn decode_indices(
        &mut self,
        triangle_count: usize,
        index_buf: &mut &[u8],
        index_type_buf: &mut &[u8],
    ) {
        let mut a = 0;
        let mut b = 0;
        let mut c = 0;
        let mut last_index = 0;

        let mut used_vertex_count = -1;
        for i in 0..triangle_count {
            let index_type = index_type_buf.g1();
            match index_type {
                1 => {
                    a = index_buf.get_smart_1_or_2s() + last_index;
                    b = index_buf.get_smart_1_or_2s() + a;
                    c = index_buf.get_smart_1_or_2s() + b;
                    last_index = c;
                    self.triangle_a[i] = a as u16;
                    self.triangle_b[i] = b as u16;
                    self.triangle_c[i] = c as u16;
                    if a > used_vertex_count {
                        used_vertex_count = a;
                    }
                    if b > used_vertex_count {
                        used_vertex_count = b;
                    }
                    if c > used_vertex_count {
                        used_vertex_count = c;
                    }
                }
                2 => {
                    b = c;
                    c = index_buf.get_smart_1_or_2s() + last_index;
                    last_index = c;
                    self.triangle_a[i] = a as u16;
                    self.triangle_b[i] = b as u16;
                    self.triangle_c[i] = c as u16;
                    if c > used_vertex_count {
                        used_vertex_count = c;
                    }
                }
                3 => {
                    a = c;
                    c = index_buf.get_smart_1_or_2s() + last_index;
                    last_index = c;
                    self.triangle_a[i] = a as u16;
                    self.triangle_b[i] = b as u16;
                    self.triangle_c[i] = c as u16;
                    if c > used_vertex_count {
                        used_vertex_count = c;
                    }
                }
                4 => {
                    let temp = a;
                    a = b;
                    b = temp;
                    c = index_buf.get_smart_1_or_2s() + last_index;
                    last_index = c;
                    self.triangle_a[i] = a as u16;
                    self.triangle_b[i] = temp as u16;
                    self.triangle_c[i] = c as u16;
                    if c > used_vertex_count {
                        used_vertex_count = c;
                    }
                }
                _ => {}
            }
        }
        used_vertex_count += 1;

        self.used_vertex_count = used_vertex_count as u16;
    }

    fn decode_texture_mapping(
        &mut self,
        textured_triangle_count: usize,
        texture_mapping_buf: &mut &[u8],
    ) {
        if textured_triangle_count > 0 {
            let texture_props = self.texture_props.as_mut().unwrap();
            for i in 0..textured_triangle_count {
                texture_props.render_types[i] = 0;
                texture_props.mapping_p[i] = texture_mapping_buf.g2();
                texture_props.mapping_m[i] = texture_mapping_buf.g2();
                texture_props.mapping_n[i] = texture_mapping_buf.g2();
            }
        }
    }

    fn decode_v1_maya(&mut self, data: &[u8]) {
        // println!("v3");
        let mut buf1 = data;
        let mut buf2 = data;
        let mut buf3 = data;
        let mut buf4 = data;
        let mut buf5 = data;
        let mut buf6 = data;
        let mut buf7 = data;
        buf1 = &data[(data.len() - 26)..];
        let vertex_count = buf1.g2() as usize;
        let triangle_count = buf1.g2() as usize;
        let textured_triangle_count = buf1.g1() as usize;
        let flags = buf1.g1();
        let has_triangle_render_types = flags & 0x1 != 0;
        let priority = buf1.g1();
        let has_priorities = priority == 255;
        let has_transparencies = buf1.g1() == 1;
        let has_triangle_skins = buf1.g1() == 1;
        let has_textures = buf1.g1() == 1;
        let has_vertex_skins = buf1.g1() == 1;
        let has_maya_groups = buf1.g1() == 1;
        let vertex_x_count = buf1.g2() as usize;
        let vertex_y_count = buf1.g2() as usize;
        let vertex_z_count = buf1.g2() as usize;
        let index_count = buf1.g2() as usize;
        let texture_coords_size = buf1.g2() as usize;
        let vertex_skins_size = buf1.g2() as usize;

        if textured_triangle_count > 0 {
            self.texture_props = Some(ModelTextureMappingProps::new(textured_triangle_count));
        }

        let (
            simple_texture_triangle_count,
            complex_texture_triangle_count,
            cube_texture_triangle_count,
        ) = self.decode_texture_render_types(textured_triangle_count, &data);

        let mut offset = textured_triangle_count;
        let vertex_flags_offset = offset;
        offset += vertex_count;
        let triangle_render_types_offset = offset;
        if has_triangle_render_types {
            offset += triangle_count;
        }
        let index_types_offset = offset;
        offset += triangle_count;
        let priorities_offset = offset;
        if has_priorities {
            offset += triangle_count;
        }
        let triangle_skins_offset = offset;
        if has_triangle_skins {
            offset += triangle_count;
        }
        let vertex_skins_offset = offset;
        offset += vertex_skins_size;
        let transparencies_offset = offset;
        if has_transparencies {
            offset += triangle_count;
        }
        let indices_offset = offset;
        offset += index_count;
        let textures_offset = offset;
        if has_textures {
            offset += triangle_count * 2;
        }
        let texture_coords_offset = offset;
        offset += texture_coords_size;
        let colours_offset = offset;
        offset += triangle_count * 2;
        let vertex_x_offset = offset;
        offset += vertex_x_count;
        let vertex_y_offset = offset;
        offset += vertex_y_count;
        let vertex_z_offset = offset;
        offset += vertex_z_count;
        let simple_textures_offset = offset;
        offset += simple_texture_triangle_count * 6;
        let complex_textures_offset = offset;
        offset += complex_texture_triangle_count * 6;
        let texture_scales_offset = offset;
        offset += complex_texture_triangle_count * 6;
        let texture_rotations_offset = offset;
        offset += complex_texture_triangle_count * 2;
        let texture_directions_offset = offset;
        offset += complex_texture_triangle_count * 2;
        let texture_translations_offset = offset;
        offset += complex_texture_triangle_count * 2 + cube_texture_triangle_count * 2;

        self.vertex_count = vertex_count as u16;
        self.triangle_count = triangle_count as u16;
        self.textured_triangle_count = textured_triangle_count as u16;
        self.vertex_x = Arc::new(vec![0; vertex_count]);
        self.vertex_y = Arc::new(vec![0; vertex_count]);
        self.vertex_z = Arc::new(vec![0; vertex_count]);
        self.triangle_a = vec![0; triangle_count];
        self.triangle_b = vec![0; triangle_count];
        self.triangle_c = vec![0; triangle_count];

        self.triangle_colour = vec![0; triangle_count];

        if has_vertex_skins {
            self.vertex_skins = Some(vec![0; vertex_count]);
        }
        if has_triangle_render_types {
            self.triangle_render_type = Some(vec![0; triangle_count]);
        }
        if has_priorities {
            self.triangle_priority = Some(vec![0; triangle_count]);
        } else {
            self.priority = priority;
        }
        if has_transparencies {
            self.triangle_transparency = Some(vec![0; triangle_count]);
        }
        if has_triangle_skins {
            self.triangle_skins = Some(vec![0; triangle_count]);
        }
        if has_textures {
            self.triangle_material = Some(vec![0; triangle_count]);
            if textured_triangle_count > 0 {
                self.triangle_texture_coords = Some(vec![0; triangle_count]);
            }
        }
        if has_maya_groups {
            self.anim_maya_props = Some(ModelAnimMayaProps::new(vertex_count));
        }

        buf1 = &data[vertex_flags_offset..];
        buf2 = &data[vertex_x_offset..];
        buf3 = &data[vertex_y_offset..];
        buf4 = &data[vertex_z_offset..];
        buf5 = &data[vertex_skins_offset..];

        self.decode_vertices(
            vertex_count,
            has_vertex_skins,
            false,
            has_maya_groups,
            &mut buf1,
            &mut buf2,
            &mut buf3,
            &mut buf4,
            &mut buf5,
        );

        buf1 = &data[colours_offset..];
        buf2 = &data[triangle_render_types_offset..];
        buf3 = &data[priorities_offset..];
        buf4 = &data[transparencies_offset..];
        buf5 = &data[triangle_skins_offset..];
        buf6 = &data[textures_offset..];
        buf7 = &data[texture_coords_offset..];

        self.decode_triangles_v1(
            triangle_count,
            has_triangle_render_types,
            has_priorities,
            has_transparencies,
            has_triangle_skins,
            has_textures,
            &mut buf1,
            &mut buf2,
            &mut buf3,
            &mut buf4,
            &mut buf5,
            &mut buf6,
            &mut buf7,
        );

        buf1 = &data[indices_offset..];
        buf2 = &data[index_types_offset..];

        self.decode_indices(triangle_count, &mut buf1, &mut buf2);

        buf1 = &data[simple_textures_offset..];
        buf2 = &data[complex_textures_offset..];
        buf3 = &data[texture_scales_offset..];
        buf4 = &data[texture_rotations_offset..];
        buf5 = &data[texture_directions_offset..];
        buf6 = &data[texture_translations_offset..];

        self.decode_texture_mapping_v1(
            textured_triangle_count,
            &mut buf1,
            &mut buf2,
            &mut buf3,
            &mut buf4,
            &mut buf5,
            &mut buf6,
        );
    }

    pub fn decode_texture_mapping_v1(
        &mut self,
        textured_triangle_count: usize,
        simple_buf: &mut &[u8],
        complex_buf: &mut &[u8],
        scales_buf: &mut &[u8],
        rotation_buf: &mut &[u8],
        direction_buf: &mut &[u8],
        translation_buf: &mut &[u8],
    ) {
        if textured_triangle_count > 0 {
            let texture_props = self.texture_props.as_mut().unwrap();
            for i in 0..textured_triangle_count {
                let texture_render_type = texture_props.render_types[i];
                if texture_render_type == 0 {
                    texture_props.mapping_p[i] = simple_buf.g2();
                    texture_props.mapping_m[i] = simple_buf.g2();
                    texture_props.mapping_n[i] = simple_buf.g2();
                } else if texture_render_type >= 1 && texture_render_type <= 3 {
                    texture_props.mapping_p[i] = complex_buf.g2();
                    texture_props.mapping_m[i] = complex_buf.g2();
                    texture_props.mapping_n[i] = complex_buf.g2();
                }
            }
        }
    }

    pub fn decode_triangles_v1(
        &mut self,
        triangle_count: usize,
        has_triangle_render_types: bool,
        has_priorities: bool,
        has_transparencies: bool,
        has_triangle_skins: bool,
        has_textures: bool,
        colour_buf: &mut &[u8],
        triangle_render_type_buf: &mut &[u8],
        priority_buf: &mut &[u8],
        transparency_buf: &mut &[u8],
        triangle_skin_buf: &mut &[u8],
        texture_buf: &mut &[u8],
        texture_coord_buf: &mut &[u8],
    ) {
        for i in 0..triangle_count {
            self.triangle_colour[i] = colour_buf.g2();
        }
        if has_triangle_render_types {
            let triangle_render_types = self.triangle_render_type.as_mut().unwrap();
            for i in 0..triangle_count {
                triangle_render_types[i] = triangle_render_type_buf.g1();
            }
        }
        if has_priorities {
            let triangle_priorities = self.triangle_priority.as_mut().unwrap();
            for i in 0..triangle_count {
                triangle_priorities[i] = priority_buf.g1();
            }
        }
        if has_transparencies {
            let triangle_transparencies = self.triangle_transparency.as_mut().unwrap();
            for i in 0..triangle_count {
                triangle_transparencies[i] = transparency_buf.g1();
            }
        }
        if has_triangle_skins {
            let triangle_skins = self.triangle_skins.as_mut().unwrap();
            for i in 0..triangle_count {
                triangle_skins[i] = triangle_skin_buf.g1() as i32;
            }
        }
        if has_textures {
            let triangle_textures = self.triangle_material.as_mut().unwrap();
            for i in 0..triangle_count {
                triangle_textures[i] = (texture_buf.g2() as i16) - 1;
            }
            if let Some(triangle_texture_coords) = self.triangle_texture_coords.as_mut() {
                for i in 0..triangle_count {
                    if triangle_textures[i] != -1 {
                        triangle_texture_coords[i] = (texture_coord_buf.g1() as i16) - 1;
                    } else {
                        triangle_texture_coords[i] = -1;
                    }
                }
            }
        }
    }

    pub fn decode_texture_render_types(
        &mut self,
        textured_triangle_count: usize,
        mut buf: &[u8],
    ) -> (usize, usize, usize) {
        let mut simple_texture_triangle_count = 0;
        let mut complex_texture_triangle_count = 0;
        let mut cube_texture_triangle_count = 0;
        if textured_triangle_count > 0 {
            let texture_props = self.texture_props.as_mut().unwrap();
            for i in 0..textured_triangle_count {
                let texture_render_type = buf.g1();
                texture_props.render_types[i] = texture_render_type;
                if texture_render_type == 0 {
                    simple_texture_triangle_count += 1;
                }
                if texture_render_type >= 1 && texture_render_type <= 3 {
                    complex_texture_triangle_count += 1;
                }
                if texture_render_type == 2 {
                    cube_texture_triangle_count += 1;
                }
            }
        }

        (
            simple_texture_triangle_count,
            complex_texture_triangle_count,
            cube_texture_triangle_count,
        )
    }

    pub fn translate(&mut self, x: i32, y: i32, z: i32) {
        let vertex_x = Arc::get_mut(&mut self.vertex_x).unwrap();
        let vertex_y = Arc::get_mut(&mut self.vertex_y).unwrap();
        let vertex_z = Arc::get_mut(&mut self.vertex_z).unwrap();
        for i in 0..self.vertex_count as usize {
            vertex_x[i] += x;
            vertex_y[i] += y;
            vertex_z[i] += z;
        }
    }

    pub fn scale_log2(&mut self, scale: i32) {
        let vertex_x = Arc::get_mut(&mut self.vertex_x).unwrap();
        let vertex_y = Arc::get_mut(&mut self.vertex_y).unwrap();
        let vertex_z = Arc::get_mut(&mut self.vertex_z).unwrap();
        for i in 0..self.vertex_count as usize {
            vertex_x[i] <<= scale;
            vertex_y[i] <<= scale;
            vertex_z[i] <<= scale;
        }
        if self.textured_triangle_count > 0 {
            if let (Some(props), Some(complex_props)) = (
                self.texture_props.as_ref(),
                self.texture_complex_props.as_mut(),
            ) {
                for i in 0..self.textured_triangle_count as usize {
                    complex_props.scale_x[i] <<= scale;
                    complex_props.scale_y[i] <<= scale;
                    if props.render_types[i] != 1 {
                        complex_props.scale_z[i] <<= scale;
                    }
                }
            }
        }
    }

    fn calculate_normals(&self) -> (Vec<VertexNormal>, Vec<TriangleNormal>) {
        let mut vertex_normals = vec![VertexNormal::default(); self.used_vertex_count as usize];
        let mut triangle_normals = vec![TriangleNormal::default(); self.triangle_count as usize];

        for t in 0..self.triangle_count as usize {
            let a = self.triangle_a[t] as usize;
            let b = self.triangle_b[t] as usize;
            let c = self.triangle_c[t] as usize;

            let delta_x0 = self.vertex_x[b] - self.vertex_x[a];
            let delta_y0 = self.vertex_y[b] - self.vertex_y[a];
            let delta_z0 = self.vertex_z[b] - self.vertex_z[a];
            let delta_x1 = self.vertex_x[c] - self.vertex_x[a];
            let delta_y1 = self.vertex_y[c] - self.vertex_y[a];
            let delta_z1 = self.vertex_z[c] - self.vertex_z[a];

            let mut nx = delta_y0 * delta_z1 - delta_y1 * delta_z0;
            let mut ny = delta_z0 * delta_x1 - delta_z1 * delta_x0;
            let mut nz = delta_x0 * delta_y1 - delta_x1 * delta_y0;
            while nx > 8192 || ny > 8192 || nz > 8192 || nx < -8192 || ny < -8192 || nz < -8192 {
                nx >>= 1;
                ny >>= 1;
                nz >>= 1;
            }

            let mut nmag = f64::sqrt((nx * nx + ny * ny + nz * nz) as f64) as i32;
            if nmag <= 0 {
                nmag = 1;
            }

            nx = nx * 256 / nmag;
            ny = ny * 256 / nmag;
            nz = nz * 256 / nmag;

            let render_type = self.triangle_render_type.as_ref().map_or(0, |rts| rts[t]);
            if render_type == 0 {
                let mut normal = &mut vertex_normals[a];
                normal.x += nx;
                normal.y += ny;
                normal.z += nz;
                normal.magnitude += 1;
                normal = &mut vertex_normals[b];
                normal.x += nx;
                normal.y += ny;
                normal.z += nz;
                normal.magnitude += 1;
                normal = &mut vertex_normals[c];
                normal.x += nx;
                normal.y += ny;
                normal.z += nz;
                normal.magnitude += 1;
            } else if render_type == 1 {
                let normal = &mut triangle_normals[t];
                normal.x = nx;
                normal.y = ny;
                normal.z = nz;
            }
        }

        (vertex_normals, triangle_normals)
    }
}

#[derive(Debug, Clone, Default)]
pub struct VertexNormal {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub magnitude: i32,
}

#[derive(Debug, Clone, Default)]
pub struct TriangleNormal {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

fn adjust_lightness(hsl: u16, lightness: i32) -> u16 {
    let mut new_lightness = (hsl & 0x7f) as i32 * lightness >> 7;
    if new_lightness < 2 {
        new_lightness = 2;
    } else if new_lightness > 126 {
        new_lightness = 126;
    }

    (hsl & 0xff80) | new_lightness as u16
}

fn clamp_lightness(lightness: i32) -> i32 {
    if lightness < 2 {
        2
    } else if lightness > 126 {
        126
    } else {
        lightness
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ModelFlags: u32 {
        const CHANGED_X = 1 << 0;
        const CHANGED_Y = 1 << 1;
        const CHANGED_Z = 1 << 2;
        const ROTATED = 1 << 3;
        const MIRRORED = 1 << 4;
        const ANIMATED_POSITION = 1 << 5;
        const ANIMATED_COLOUR = 1 << 7;
        const ANIMATED_TRANSPARENCY = 1 << 8;
        const ANIMATED_NORMAL = 1 << 9;
        const ANIMATED_BILLBOARD = 1 << 10;
        const RENDER = 1 << 11;
        const CHANGED_AMBIENT = 1 << 12;
        const CHANGED_CONTRAST = 1 << 13;
        const RECOLOURED = 1 << 14;
        const RETEXTURED = 1 << 15;
        const MERGE_NORMALS = 1 << 16;
        const CASTS_SHADOW = 1 << 19;
        const CHANGED_AMBIENT_COLOUR = 1 << 20;
    }
}

impl ModelFlags {
    pub fn has_changed_x(&self) -> bool {
        self.intersects(Self::CHANGED_X | Self::ANIMATED_POSITION)
    }

    pub fn has_changed_y(&self) -> bool {
        self.intersects(Self::CHANGED_Y | Self::ANIMATED_POSITION)
    }

    pub fn has_changed_z(&self) -> bool {
        self.intersects(Self::CHANGED_Z | Self::MIRRORED | Self::ANIMATED_POSITION)
    }

    pub fn has_changed_colour(&self) -> bool {
        self.intersects(Self::ANIMATED_COLOUR | Self::RECOLOURED | Self::CHANGED_AMBIENT_COLOUR)
    }

    pub fn has_changed_transparency(&self) -> bool {
        self.intersects(Self::ANIMATED_TRANSPARENCY)
    }

    pub fn has_changed_material(&self) -> bool {
        self.intersects(Self::RETEXTURED)
    }

    pub fn has_changed_indices(&self) -> bool {
        self.intersects(Self::MIRRORED)
    }

    pub fn has_changed_normals(&self) -> bool {
        self.contains(Self::ANIMATED_POSITION | Self::ANIMATED_NORMAL)
            || self.intersects(Self::ROTATED | Self::MIRRORED)
    }

    pub fn has_changed_texcoords(&self) -> bool {
        false
    }
}

pub struct ModelRenderVertices {
    pub vertex_stream_pos: Vec<u16>,
    pub normal_x: Vec<i16>,
    pub normal_y: Vec<i16>,
    pub normal_z: Vec<i16>,
    pub normal_magnitude: Vec<i8>,
    pub texcoord_u: Vec<f32>,
    pub texcoord_v: Vec<f32>,
    pub render_vertex_count: u16,
}

impl ModelRenderVertices {
    pub fn new(render_vertex_capacity: usize) -> Self {
        Self {
            vertex_stream_pos: vec![0; render_vertex_capacity],
            normal_x: vec![0; render_vertex_capacity],
            normal_y: vec![0; render_vertex_capacity],
            normal_z: vec![0; render_vertex_capacity],
            normal_magnitude: vec![0; render_vertex_capacity],
            texcoord_u: vec![0.0; render_vertex_capacity],
            texcoord_v: vec![0.0; render_vertex_capacity],
            render_vertex_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub min_x: i32,
    pub min_y: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_y: i32,
    pub max_z: i32,
}

impl BoundingBox {
    pub fn get_center(&self) -> (i32, i32, i32) {
        (
            (self.min_x + self.max_x) / 2,
            (self.min_y + self.max_y) / 2,
            (self.min_z + self.max_z) / 2,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModelBounds {
    pub bounding_box: BoundingBox,
    pub xz_radius: i32,
    pub xyz_radius: i32,
}

#[derive(Debug)]
pub struct ModelLit {
    pub flags: ModelFlags,
    pub ambient: i16,
    pub contrast: i16,
    pub vertex_count: u16,
    pub used_vertex_count: u16,
    pub render_vertex_count: u16,
    pub triangle_count: u16,
    pub render_triangle_count: u16,
    pub is_transparent: bool,
    pub vertex_unique_index: Arc<Vec<u32>>,
    pub vertex_x: Arc<Vec<i32>>,
    pub vertex_y: Arc<Vec<i32>>,
    pub vertex_z: Arc<Vec<i32>>,
    pub vertex_stream_pos: Arc<Vec<u16>>,
    pub normal_x: Arc<Vec<i16>>,
    pub normal_y: Arc<Vec<i16>>,
    pub normal_z: Arc<Vec<i16>>,
    pub normal_magnitude: Arc<Vec<i8>>,
    pub texcoord_u: Arc<Vec<f32>>,
    pub texcoord_v: Arc<Vec<f32>>,
    // TODO: can be removed maybe
    pub triangle_render_type: Arc<Vec<u8>>,
    pub triangle_colour: Arc<Vec<u16>>,
    pub triangle_transparency: Arc<Vec<u8>>,
    pub triangle_material: Arc<Vec<i16>>,
    pub triangle_render_a: Arc<Vec<u16>>,
    pub triangle_render_b: Arc<Vec<u16>>,
    pub triangle_render_c: Arc<Vec<u16>>,
    // TODO: Move to bounds struct?
    pub bounds: Option<ModelBounds>,
}

impl ModelLit {
    pub fn new() -> Self {
        Self {
            flags: ModelFlags::empty(),
            ambient: 0,
            contrast: 0,
            vertex_count: 0,
            used_vertex_count: 0,
            render_vertex_count: 0,
            triangle_count: 0,
            render_triangle_count: 0,
            is_transparent: false,
            vertex_unique_index: Arc::new(Vec::new()),
            vertex_x: Arc::new(Vec::new()),
            vertex_y: Arc::new(Vec::new()),
            vertex_z: Arc::new(Vec::new()),
            vertex_stream_pos: Arc::new(Vec::new()),
            normal_x: Arc::new(Vec::new()),
            normal_y: Arc::new(Vec::new()),
            normal_z: Arc::new(Vec::new()),
            normal_magnitude: Arc::new(Vec::new()),
            texcoord_u: Arc::new(Vec::new()),
            texcoord_v: Arc::new(Vec::new()),
            triangle_render_type: Arc::new(Vec::new()),
            triangle_colour: Arc::new(Vec::new()),
            triangle_transparency: Arc::new(Vec::new()),
            triangle_material: Arc::new(Vec::new()),
            triangle_render_a: Arc::new(Vec::new()),
            triangle_render_b: Arc::new(Vec::new()),
            triangle_render_c: Arc::new(Vec::new()),
            bounds: None,
        }
    }

    pub fn from_unlit(
        texture_provider: &TextureProvider,
        model: &ModelUnlit,
        flags: ModelFlags,
        ambient: i16,
        contrast: i16,
    ) -> Self {
        let mut is_transparent = false;
        let mut triangle_indices = Vec::with_capacity(model.triangle_count as usize);
        let mut vertex_unique_index = vec![0u32; model.used_vertex_count as usize + 1];
        // TODO: get from render flags
        let hd_textures_enabled = true;
        for t in 0..model.triangle_count as usize {
            let render_type = model.triangle_render_type.as_ref().map_or(0, |ts| ts[t]);
            if render_type == 2 {
                continue;
            }
            let material_id = model.triangle_material.as_ref().map_or(-1, |ts| ts[t]);
            if material_id != -1 {
                let info = texture_provider
                    .get_info(material_id as u16 as u32)
                    .unwrap_or_default();
                if (hd_textures_enabled || !info.high_detail) && info.standard_detail_only {
                    continue;
                }
            }
            triangle_indices.push(t as u16);
            vertex_unique_index[model.triangle_a[t] as usize] += 1;
            vertex_unique_index[model.triangle_b[t] as usize] += 1;
            vertex_unique_index[model.triangle_c[t] as usize] += 1;
        }
        let triangle_count = triangle_indices.len();
        let render_triangle_count = triangle_count;
        let mut sort_keys = vec![0u64; model.triangle_count as usize];
        let is_model_transparent = flags.contains(ModelFlags::ANIMATED_TRANSPARENCY);
        for i in 0..triangle_count {
            let t = triangle_indices[i] as usize;
            let mut key = 0u64;
            let mut texture_id = model.triangle_material.as_ref().map_or(-1, |ts| ts[t]);
            let mut material_info = None;
            if texture_id != -1 {
                let info = texture_provider
                    .get_info(texture_id as u16 as u32)
                    .unwrap_or_default();
                if !hd_textures_enabled && info.high_detail {
                    texture_id = -1;
                } else {
                    material_info = Some(info);
                }
            }
            let (effect_id, effect_config0, is_material_transparent) =
                material_info.as_ref().map_or((0, 0, false), |info| {
                    (
                        info.effect_id,
                        info.effect_config0,
                        info.alpha_mode != AlphaMode::Opaque,
                    )
                });
            let is_triangle_transparent = model
                .triangle_transparency
                .as_ref()
                .map_or(false, |ts| ts[t] != 0)
                || is_material_transparent;
            if is_model_transparent || is_triangle_transparent {
                if let Some(priorities) = &model.triangle_priority {
                    key |= (priorities[t] as u64) << 49;
                }
            }

            if is_triangle_transparent {
                key |= 1 << 48;
            }
            key |= (effect_id as u64) << 40;
            key |= (effect_config0 as u64) << 32;
            key |= (texture_id as u16 as u64) << 16;
            key |= i as u64 & 0xffff;
            sort_keys[t] = key;
            is_transparent |= is_triangle_transparent;
        }
        triangle_indices.sort_by_key(|i| sort_keys[*i as usize]);

        let render_vertex_capacity = triangle_count * 3;
        let mut render_vertices = ModelRenderVertices::new(render_vertex_capacity);

        let mut triangle_render_type = vec![0u8; triangle_count];
        let mut triangle_colour = vec![0u16; triangle_count];
        let mut triangle_transparency = vec![0u8; triangle_count];
        let mut triangle_material = vec![0i16; triangle_count];
        let mut triangle_render_a = vec![0u16; triangle_count];
        let mut triangle_render_b = vec![0u16; triangle_count];
        let mut triangle_render_c = vec![0u16; triangle_count];

        let mut vertex_data_index = 0;
        for v in 0..model.used_vertex_count as usize {
            let vertex_ref_count = vertex_unique_index[v];
            vertex_unique_index[v] = vertex_data_index;
            vertex_data_index += vertex_ref_count;
        }
        vertex_unique_index[model.used_vertex_count as usize] = vertex_data_index;

        let (vertex_normals, triangle_normals) = model.calculate_normals();

        for i in 0..triangle_count {
            let t = triangle_indices[i] as usize;
            let colour_hsl = model.triangle_colour[t];
            let mut texture_coord = model
                .triangle_texture_coords
                .as_ref()
                .map_or(-1, |coords| coords[t] as i32);
            let transparency = model
                .triangle_transparency
                .as_ref()
                .map_or(0, |transparencies| transparencies[t]);
            let texture_id = model
                .triangle_material
                .as_ref()
                .map_or(-1, |textures| textures[t]);
            let mut u0 = 0f32;
            let mut v0 = 0f32;
            let mut u1 = 0f32;
            let mut v1 = 0f32;
            let mut u2 = 0f32;
            let mut v2 = 0f32;
            if texture_id != -1 {
                if texture_coord == 32766 {
                } else {
                    let mut mapping_type = 0;
                    if texture_coord != -1 {
                        texture_coord &= 0xffff;
                        mapping_type = model
                            .texture_props
                            .as_ref()
                            .map_or(0, |tp| tp.render_types[texture_coord as usize]);
                    }
                    let a = model.triangle_a[t] as usize;
                    let b = model.triangle_b[t] as usize;
                    let c = model.triangle_c[t] as usize;
                    if mapping_type == 0 {
                        let mut p = a;
                        let mut m = b;
                        let mut n = c;
                        if texture_coord != -1 {
                            let props = model.texture_props.as_ref().unwrap();
                            p = props.mapping_p[texture_coord as usize] as usize;
                            m = props.mapping_m[texture_coord as usize] as usize;
                            n = props.mapping_n[texture_coord as usize] as usize;
                        }

                        let origin_x = model.vertex_x[p] as f32;
                        let origin_y = model.vertex_y[p] as f32;
                        let origin_z = model.vertex_z[p] as f32;

                        let m_delta_x = model.vertex_x[m] as f32 - origin_x;
                        let m_delta_y = model.vertex_y[m] as f32 - origin_y;
                        let m_delta_z = model.vertex_z[m] as f32 - origin_z;
                        let n_delta_x = model.vertex_x[n] as f32 - origin_x;
                        let n_delta_y = model.vertex_y[n] as f32 - origin_y;
                        let n_delta_z = model.vertex_z[n] as f32 - origin_z;
                        let a_delta_x = model.vertex_x[a] as f32 - origin_x;
                        let a_delta_y = model.vertex_y[a] as f32 - origin_y;
                        let a_delta_z = model.vertex_z[a] as f32 - origin_z;
                        let b_delta_x = model.vertex_x[b] as f32 - origin_x;
                        let b_delta_y = model.vertex_y[b] as f32 - origin_y;
                        let b_delta_z = model.vertex_z[b] as f32 - origin_z;
                        let c_delta_x = model.vertex_x[c] as f32 - origin_x;
                        let c_delta_y = model.vertex_y[c] as f32 - origin_y;
                        let c_delta_z = model.vertex_z[c] as f32 - origin_z;

                        let f_897_ = m_delta_y * n_delta_z - n_delta_y * m_delta_z;
                        let f_898_ = n_delta_x * m_delta_z - m_delta_x * n_delta_z;
                        let f_899_ = m_delta_x * n_delta_y - n_delta_x * m_delta_y;
                        let mut f_900_ = n_delta_y * f_899_ - n_delta_z * f_898_;
                        let mut f_901_ = n_delta_z * f_897_ - n_delta_x * f_899_;
                        let mut f_902_ = n_delta_x * f_898_ - n_delta_y * f_897_;
                        let mut f_903_ =
                            1.0 / (f_900_ * m_delta_x + f_901_ * m_delta_y + f_902_ * m_delta_z);

                        u0 =
                            (f_900_ * a_delta_x + f_901_ * a_delta_y + f_902_ * a_delta_z) * f_903_;
                        u1 =
                            (f_900_ * b_delta_x + f_901_ * b_delta_y + f_902_ * b_delta_z) * f_903_;
                        u2 =
                            (f_900_ * c_delta_x + f_901_ * c_delta_y + f_902_ * c_delta_z) * f_903_;

                        f_900_ = m_delta_y * f_899_ - m_delta_z * f_898_;
                        f_901_ = m_delta_z * f_897_ - m_delta_x * f_899_;
                        f_902_ = m_delta_x * f_898_ - m_delta_y * f_897_;
                        f_903_ =
                            1.0 / (f_900_ * n_delta_x + f_901_ * n_delta_y + f_902_ * n_delta_z);

                        v0 =
                            (f_900_ * a_delta_x + f_901_ * a_delta_y + f_902_ * a_delta_z) * f_903_;
                        v1 =
                            (f_900_ * b_delta_x + f_901_ * b_delta_y + f_902_ * b_delta_z) * f_903_;
                        v2 =
                            (f_900_ * c_delta_x + f_901_ * c_delta_y + f_902_ * c_delta_z) * f_903_;
                    }
                }
            }

            let render_type = model.triangle_render_type.as_ref().map_or(0, |rts| rts[t]);
            if render_type == 0 {
                let a = model.triangle_a[t];
                let b = model.triangle_b[t];
                let c = model.triangle_c[t];
                let mut normal = &vertex_normals[a as usize];
                triangle_render_a[i] = Self::add_render_vertex(
                    &vertex_unique_index,
                    &mut render_vertices,
                    a,
                    normal.x,
                    normal.y,
                    normal.z,
                    normal.magnitude,
                    u0,
                    v0,
                );
                normal = &vertex_normals[b as usize];
                triangle_render_b[i] = Self::add_render_vertex(
                    &vertex_unique_index,
                    &mut render_vertices,
                    b,
                    normal.x,
                    normal.y,
                    normal.z,
                    normal.magnitude,
                    u1,
                    v1,
                );
                normal = &vertex_normals[c as usize];
                triangle_render_c[i] = Self::add_render_vertex(
                    &vertex_unique_index,
                    &mut render_vertices,
                    c,
                    normal.x,
                    normal.y,
                    normal.z,
                    normal.magnitude,
                    u2,
                    v2,
                );
            } else if render_type == 1 {
                let normal = &triangle_normals[t];
                triangle_render_a[i] = Self::add_render_vertex(
                    &vertex_unique_index,
                    &mut render_vertices,
                    model.triangle_a[t],
                    normal.x,
                    normal.y,
                    normal.z,
                    0,
                    u0,
                    v0,
                );
                triangle_render_b[i] = Self::add_render_vertex(
                    &vertex_unique_index,
                    &mut render_vertices,
                    model.triangle_b[t],
                    normal.x,
                    normal.y,
                    normal.z,
                    0,
                    u1,
                    v1,
                );
                triangle_render_c[i] = Self::add_render_vertex(
                    &vertex_unique_index,
                    &mut render_vertices,
                    model.triangle_c[t],
                    normal.x,
                    normal.y,
                    normal.z,
                    0,
                    u2,
                    v2,
                );
            }

            triangle_render_type[i] = render_type;
            triangle_colour[i] = colour_hsl;
            triangle_transparency[i] = transparency;
            triangle_material[i] = texture_id;
        }
        // TODO: truncate
        // self.normal_x.truncate(self.render_triangle_count as usize);
        // self.normal_y.truncate(self.render_triangle_count as usize);
        // self.normal_z.truncate(self.render_triangle_count as usize);
        // self.normal_magnitude.truncate(self.render_triangle_count as usize);
        // self.texcoord_u.truncate(self.render_triangle_count as usize);
        // self.texcoord_v.truncate(self.render_triangle_count as usize);

        Self {
            flags,
            ambient,
            contrast,
            vertex_count: model.vertex_count,
            used_vertex_count: model.used_vertex_count,
            render_vertex_count: render_vertices.render_vertex_count,
            triangle_count: triangle_count as u16,
            render_triangle_count: render_triangle_count as u16,
            is_transparent,
            vertex_unique_index: Arc::new(vertex_unique_index),
            vertex_x: model.vertex_x.clone(),
            vertex_y: model.vertex_y.clone(),
            vertex_z: model.vertex_z.clone(),
            vertex_stream_pos: Arc::new(render_vertices.vertex_stream_pos),
            normal_x: Arc::new(render_vertices.normal_x),
            normal_y: Arc::new(render_vertices.normal_y),
            normal_z: Arc::new(render_vertices.normal_z),
            normal_magnitude: Arc::new(render_vertices.normal_magnitude),
            texcoord_u: Arc::new(render_vertices.texcoord_u),
            texcoord_v: Arc::new(render_vertices.texcoord_v),
            triangle_render_type: Arc::new(triangle_render_type),
            triangle_colour: Arc::new(triangle_colour),
            triangle_transparency: Arc::new(triangle_transparency),
            triangle_material: Arc::new(triangle_material),
            triangle_render_a: Arc::new(triangle_render_a),
            triangle_render_b: Arc::new(triangle_render_b),
            triangle_render_c: Arc::new(triangle_render_c),
            bounds: None,
        }
    }

    pub fn add_render_vertex(
        vertex_unique_index: &[u32],
        vertices: &mut ModelRenderVertices,
        vertex_pos_index: u16,
        normal_x: i32,
        normal_y: i32,
        normal_z: i32,
        normal_magnitude: i32,
        texcoord_u: f32,
        texcoord_v: f32,
    ) -> u16 {
        let v_start = vertex_unique_index[vertex_pos_index as usize];
        let v_end = vertex_unique_index[vertex_pos_index as usize + 1];
        let mut stream_index = 0;
        for v in v_start..v_end {
            let pos = vertices.vertex_stream_pos[v as usize];
            if pos == 0 {
                stream_index = v;
                break;
            }
            // TODO: check hash
        }
        vertices.vertex_stream_pos[stream_index as usize] = vertices.render_vertex_count + 1;

        let vertex_count = vertices.render_vertex_count as usize;
        vertices.normal_x[vertex_count] = normal_x as i16;
        vertices.normal_y[vertex_count] = normal_y as i16;
        vertices.normal_z[vertex_count] = normal_z as i16;
        vertices.normal_magnitude[vertex_count] = normal_magnitude as i8;
        vertices.texcoord_u[vertex_count] = texcoord_u;
        vertices.texcoord_v[vertex_count] = texcoord_v;

        vertices.render_vertex_count += 1;

        vertex_count as u16
    }

    pub fn set_flags(&mut self, flags: ModelFlags) {
        self.flags = flags;
    }

    pub fn translate(&mut self, x: i32, y: i32, z: i32) {
        if x != 0 {
            let vertex_x = Arc::get_mut(&mut self.vertex_x).unwrap();
            for i in 0..self.used_vertex_count as usize {
                vertex_x[i] += x;
            }
        }
        if y != 0 {
            let vertex_y = Arc::get_mut(&mut self.vertex_y).unwrap();
            for i in 0..self.used_vertex_count as usize {
                vertex_y[i] += y;
            }
        }
        if z != 0 {
            let vertex_z = Arc::get_mut(&mut self.vertex_z).unwrap();
            for i in 0..self.used_vertex_count as usize {
                vertex_z[i] += z;
            }
        }

        self.bounds = None;
    }

    pub fn scale(&mut self, x: i32, y: i32, z: i32) {
        if x != 128 {
            let vertex_x = Arc::get_mut(&mut self.vertex_x).unwrap();
            for i in 0..self.used_vertex_count as usize {
                vertex_x[i] = vertex_x[i] * x >> 7;
            }
        }
        if y != 128 {
            let vertex_y = Arc::get_mut(&mut self.vertex_y).unwrap();
            for i in 0..self.used_vertex_count as usize {
                vertex_y[i] = vertex_y[i] * y >> 7;
            }
        }
        if z != 128 {
            let vertex_z = Arc::get_mut(&mut self.vertex_z).unwrap();
            for i in 0..self.used_vertex_count as usize {
                vertex_z[i] = vertex_z[i] * z >> 7;
            }
        }

        self.bounds = None;
    }

    pub fn rotate_y(&mut self, degrees: JagDegrees) {
        let sin = SINE[degrees as usize];
        let cos = COSINE[degrees as usize];
        let vertex_x = Arc::get_mut(&mut self.vertex_x).unwrap();
        let vertex_z = Arc::get_mut(&mut self.vertex_z).unwrap();
        for i in 0..self.used_vertex_count as usize {
            let x = vertex_x[i];
            let z = vertex_z[i];
            vertex_x[i] = (x * cos + z * sin) >> 14;
            vertex_z[i] = (z * cos - x * sin) >> 14;
        }
        let normal_x = Arc::get_mut(&mut self.normal_x).unwrap();
        let normal_z = Arc::get_mut(&mut self.normal_z).unwrap();
        for i in 0..self.render_vertex_count as usize {
            let x = normal_x[i] as i32;
            let z = normal_z[i] as i32;
            normal_x[i] = ((x * cos + z * sin) >> 14) as i16;
            normal_z[i] = ((z * cos - x * sin) >> 14) as i16;
        }

        self.bounds = None;
    }

    pub fn rotate_y_pos(&mut self, degrees: JagDegrees) {
        let sin = SINE[degrees as usize];
        let cos = COSINE[degrees as usize];
        let vertex_x = Arc::get_mut(&mut self.vertex_x).unwrap();
        let vertex_z = Arc::get_mut(&mut self.vertex_z).unwrap();
        for i in 0..self.used_vertex_count as usize {
            let x = vertex_x[i];
            let z = vertex_z[i];
            vertex_x[i] = (x * cos + z * sin) >> 14;
            vertex_z[i] = (z * cos - x * sin) >> 14;
        }

        self.bounds = None;
    }

    pub fn mirror(&mut self) {
        let vertex_z = Arc::get_mut(&mut self.vertex_z).unwrap();
        for i in 0..self.used_vertex_count as usize {
            vertex_z[i] = -vertex_z[i];
        }
        let normal_z = Arc::get_mut(&mut self.normal_z).unwrap();
        for i in 0..self.render_vertex_count as usize {
            normal_z[i] = -normal_z[i];
        }
        let triangle_a = Arc::get_mut(&mut self.triangle_render_a).unwrap();
        let triangle_c = Arc::get_mut(&mut self.triangle_render_c).unwrap();
        for i in 0..self.triangle_count as usize {
            std::mem::swap(&mut triangle_a[i], &mut triangle_c[i]);
        }

        self.bounds = None;
    }

    pub fn replace_colour(&mut self, old_colour: u16, new_colour: u16) {
        let triangle_colour = Arc::get_mut(&mut self.triangle_colour).unwrap();
        for i in 0..self.render_triangle_count as usize {
            if triangle_colour[i] == old_colour {
                triangle_colour[i] = new_colour;
            }
        }
    }

    pub fn replace_material(&mut self, old_material: i16, new_material: i16) {
        let triangle_material = Arc::get_mut(&mut self.triangle_material).unwrap();
        for i in 0..self.render_triangle_count as usize {
            if triangle_material[i] == old_material {
                triangle_material[i] = new_material;
            }
        }
    }

    pub fn copy(&self, flags: ModelFlags) -> Self {
        let mut copy = Self::new();
        copy.ambient = self.ambient;
        copy.contrast = self.contrast;
        copy.vertex_count = self.vertex_count;
        copy.used_vertex_count = self.used_vertex_count;
        copy.render_vertex_count = self.render_vertex_count;
        copy.triangle_count = self.triangle_count;
        copy.render_triangle_count = self.render_triangle_count;
        if flags.contains(ModelFlags::ANIMATED_TRANSPARENCY) {
            copy.is_transparent = true;
        } else {
            copy.is_transparent = self.is_transparent;
        }

        copy.vertex_unique_index = self.vertex_unique_index.clone();
        copy.vertex_stream_pos = self.vertex_stream_pos.clone();
        copy.triangle_render_type = self.triangle_render_type.clone();

        if flags.has_changed_x() {
            copy.vertex_x = Arc::new(Vec::clone(&self.vertex_x));
        } else {
            copy.vertex_x = self.vertex_x.clone();
        }
        if flags.has_changed_y() {
            copy.vertex_y = Arc::new(Vec::clone(&self.vertex_y));
        } else {
            copy.vertex_y = self.vertex_y.clone();
        }
        if flags.has_changed_z() {
            copy.vertex_z = Arc::new(Vec::clone(&self.vertex_z));
        } else {
            copy.vertex_z = self.vertex_z.clone();
        }

        if flags.has_changed_colour() {
            copy.triangle_colour = Arc::new(Vec::clone(&self.triangle_colour));
        } else {
            copy.triangle_colour = self.triangle_colour.clone();
        }
        if flags.has_changed_transparency() {
            copy.triangle_transparency = Arc::new(Vec::clone(&self.triangle_transparency));
        } else {
            copy.triangle_transparency = self.triangle_transparency.clone();
        }
        if flags.has_changed_material() {
            copy.triangle_material = Arc::new(Vec::clone(&self.triangle_material));
        } else {
            copy.triangle_material = self.triangle_material.clone();
        }
        if flags.has_changed_indices() {
            copy.triangle_render_a = Arc::new(Vec::clone(&self.triangle_render_a));
            copy.triangle_render_b = Arc::new(Vec::clone(&self.triangle_render_b));
            copy.triangle_render_c = Arc::new(Vec::clone(&self.triangle_render_c));
        } else {
            copy.triangle_render_a = self.triangle_render_a.clone();
            copy.triangle_render_b = self.triangle_render_b.clone();
            copy.triangle_render_c = self.triangle_render_c.clone();
        }

        if flags.has_changed_normals() {
            copy.normal_x = Arc::new(Vec::clone(&self.normal_x));
            copy.normal_y = Arc::new(Vec::clone(&self.normal_y));
            copy.normal_z = Arc::new(Vec::clone(&self.normal_z));
            copy.normal_magnitude = Arc::new(Vec::clone(&self.normal_magnitude));
        } else {
            copy.normal_x = self.normal_x.clone();
            copy.normal_y = self.normal_y.clone();
            copy.normal_z = self.normal_z.clone();
            copy.normal_magnitude = self.normal_magnitude.clone();
        }

        if flags.has_changed_texcoords() {
            copy.texcoord_u = Arc::new(Vec::clone(&self.texcoord_u));
            copy.texcoord_v = Arc::new(Vec::clone(&self.texcoord_v));
        } else {
            copy.texcoord_u = self.texcoord_u.clone();
            copy.texcoord_v = self.texcoord_v.clone();
        }

        copy.bounds = self.bounds.clone();

        copy
    }

    pub fn calculate_bounds(&self) -> ModelBounds {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut min_z = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut max_z = i32::MIN;
        let mut max_xz_length = 0;
        let mut max_xyz_length = 0;
        for v in 0..self.used_vertex_count as usize {
            let vx = self.vertex_x[v];
            let vy = self.vertex_y[v];
            let vz = self.vertex_z[v];
            if vx < min_x {
                min_x = vx;
            }
            if vx > max_x {
                max_x = vx;
            }
            if vy < min_y {
                min_y = vy;
            }
            if vy > max_y {
                max_y = vy;
            }
            if vz < min_z {
                min_z = vz;
            }
            if vz > max_z {
                max_z = vz;
            }
            let xz_length = vx * vx + vz * vz;
            if xz_length > max_xz_length {
                max_xz_length = xz_length;
            }
            let xyz_length = xz_length + vy * vy;
            if xyz_length > max_xyz_length {
                max_xyz_length = xyz_length;
            }
        }
        if min_x == i32::MAX {
            min_x = 0;
            min_y = 0;
            min_z = 0;
        }
        if max_x == i32::MIN {
            max_x = 0;
            max_y = 0;
            max_z = 0;
        }

        let bounding_box = BoundingBox {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
        };

        let bounds = ModelBounds {
            bounding_box,
            xz_radius: (f64::sqrt(max_xz_length as f64) + 0.99) as i32,
            xyz_radius: (f64::sqrt(max_xyz_length as f64) + 0.99) as i32,
        };

        bounds
    }

    pub fn get_xyz_radius(&mut self) -> i32 {
        if let Some(bounds) = &self.bounds {
            return bounds.xyz_radius;
        }

        self.bounds.insert(self.calculate_bounds()).xyz_radius
    }

    pub fn get_center(&mut self) -> (i32, i32, i32) {
        if let Some(bounds) = &self.bounds {
            return bounds.bounding_box.get_center();
        }

        self.bounds
            .insert(self.calculate_bounds())
            .bounding_box
            .get_center()
    }

    pub fn calc_lit_colours(
        &self,
        light_x: i32,
        light_y: i32,
        light_z: i32,
    ) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
        let ambient = self.ambient as i32;
        let contrast = self.contrast as i32;

        let light_mag =
            f64::sqrt((light_x * light_x + light_y * light_y + light_z * light_z) as f64) as i32;
        let scaled_light_mag = light_mag * contrast >> 8;

        let mut triangle_colours_a = vec![0; self.triangle_count as usize];
        let mut triangle_colours_b = vec![0; self.triangle_count as usize];
        let mut triangle_colours_c = vec![0; self.triangle_count as usize];

        for t in 0..self.triangle_count as usize {
            let mut render_type = self.triangle_render_type[t];

            let texture_id = self.triangle_material[t];

            let transparency = self.triangle_transparency[t];

            if transparency == 0xfe {
                render_type = 3;
            }

            if transparency == 0xff {
                render_type = 2;
            }

            if texture_id == -1 {
                if render_type == 0 {
                    let colour = self.triangle_colour[t];

                    let mut index = self.triangle_render_a[t] as usize;
                    let mut nx = self.normal_x[index] as i32;
                    let mut ny = self.normal_y[index] as i32;
                    let mut nz = self.normal_z[index] as i32;
                    let mut nmag = self.normal_magnitude[index] as i32;
                    let lightness = (light_x * nx + light_z * nz + light_y * ny)
                        / (scaled_light_mag * nmag)
                        + ambient;
                    triangle_colours_a[t] = adjust_lightness(colour, lightness) as i32;

                    index = self.triangle_render_b[t] as usize;
                    nx = self.normal_x[index] as i32;
                    ny = self.normal_y[index] as i32;
                    nz = self.normal_z[index] as i32;
                    nmag = self.normal_magnitude[index] as i32;
                    let lightness = (light_x * nx + light_z * nz + light_y * ny)
                        / (scaled_light_mag * nmag)
                        + ambient;
                    triangle_colours_b[t] = adjust_lightness(colour, lightness) as i32;

                    index = self.triangle_render_c[t] as usize;
                    nx = self.normal_x[index] as i32;
                    ny = self.normal_y[index] as i32;
                    nz = self.normal_z[index] as i32;
                    nmag = self.normal_magnitude[index] as i32;
                    let lightness = (light_x * nx + light_z * nz + light_y * ny)
                        / (scaled_light_mag * nmag)
                        + ambient;
                    triangle_colours_c[t] = adjust_lightness(colour, lightness) as i32;
                } else if render_type == 1 {
                    let a = self.triangle_render_a[t] as usize;
                    let nx = self.normal_x[a] as i32;
                    let ny = self.normal_y[a] as i32;
                    let nz = self.normal_z[a] as i32;
                    let lightness = (light_x * nx + light_z * nz + light_y * ny)
                        / (scaled_light_mag / 2 + scaled_light_mag)
                        + ambient;
                    triangle_colours_a[t] =
                        adjust_lightness(self.triangle_colour[t], lightness) as i32;
                    triangle_colours_c[t] = -1;
                } else if render_type == 3 {
                    triangle_colours_a[t] = 128;
                    triangle_colours_c[t] = -1;
                } else {
                    triangle_colours_c[t] = -2;
                }
            } else if render_type == 0 {
                let mut index = self.triangle_render_a[t] as usize;
                let mut nx = self.normal_x[index] as i32;
                let mut ny = self.normal_y[index] as i32;
                let mut nz = self.normal_z[index] as i32;
                let mut nmag = self.normal_magnitude[index] as i32;
                let lightness = (light_x * nx + light_z * nz + light_y * ny)
                    / (scaled_light_mag * nmag)
                    + ambient;
                triangle_colours_a[t] = clamp_lightness(lightness) as i32;

                index = self.triangle_render_b[t] as usize;
                nx = self.normal_x[index] as i32;
                ny = self.normal_y[index] as i32;
                nz = self.normal_z[index] as i32;
                nmag = self.normal_magnitude[index] as i32;
                let lightness = (light_x * nx + light_z * nz + light_y * ny)
                    / (scaled_light_mag * nmag)
                    + ambient;
                triangle_colours_b[t] = clamp_lightness(lightness) as i32;

                index = self.triangle_render_c[t] as usize;
                nx = self.normal_x[index] as i32;
                ny = self.normal_y[index] as i32;
                nz = self.normal_z[index] as i32;
                nmag = self.normal_magnitude[index] as i32;
                let lightness = (light_x * nx + light_z * nz + light_y * ny)
                    / (scaled_light_mag * nmag)
                    + ambient;
                triangle_colours_c[t] = clamp_lightness(lightness) as i32;
            } else if render_type == 1 {
                let a = self.triangle_render_a[t] as usize;
                let nx = self.normal_x[a] as i32;
                let ny = self.normal_y[a] as i32;
                let nz = self.normal_z[a] as i32;
                let lightness = (light_x * nx + light_z * nz + light_y * ny)
                    / (scaled_light_mag / 2 + scaled_light_mag)
                    + ambient;
                triangle_colours_a[t] = clamp_lightness(lightness) as i32;
                triangle_colours_c[t] = -1;
            } else {
                triangle_colours_c[t] = -2;
            }
        }

        (triangle_colours_a, triangle_colours_b, triangle_colours_c)
    }
}
