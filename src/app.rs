use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use eframe::{egui_glow, glow};
use egui::mutex::Mutex;
use wasm_bindgen::prelude::*;

use crate::runetek5::{
    graphics::{
        model::{ModelFlags, ModelLit, ModelUnlit},
        texture::TextureProvider,
    },
    js5::Js5,
};

extern crate nalgebra_glm as glm;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = performance)]
    fn now() -> f64;
}

struct ModelRenderContext {
    program: glow::Program,
    texture_array: glow::Texture,
    model_viewer: Arc<Mutex<ModelViewer>>,
}

pub struct ModelViewerApp {
    gl: Arc<glow::Context>,
    render_ctx: ModelRenderContext,
    model_js5: Arc<Js5>,
    texture_provider: TextureProvider,
    model_selector: ModelSelectorWindow,
    selected_model_id: u32,
    current_model_id: u32,
    yaw: f32,
    pitch: f32,
    zoom: f32,
}

impl ModelViewerApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        model_js5: Arc<Js5>,
        texture_provider: TextureProvider,
    ) -> Self {
        let gl = cc.gl.as_ref().unwrap().clone();
        let model_viewer = ModelViewer::new(6.0);
        let program = Self::init_shader_program(&gl);
        let texture_array = Self::init_texture_array(&gl, &texture_provider);
        let render_ctx = ModelRenderContext {
            program,
            texture_array,
            model_viewer: Arc::new(Mutex::new(model_viewer)),
        };
        Self {
            gl: gl.clone(),
            render_ctx,
            model_js5,
            texture_provider,
            model_selector: ModelSelectorWindow::new(gl.clone()),
            selected_model_id: 0,
            current_model_id: u32::MAX,
            yaw: 90.0,
            pitch: 0.0,
            zoom: 1.0,
        }
    }

    fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        if response.dragged_by(egui::PointerButton::Secondary) {
            // Add panning
        } else {
            self.yaw += response.drag_motion().x * 0.3;
            self.pitch += response.drag_motion().y * 0.3;
            if self.pitch > 89.0 {
                self.pitch = 89.0;
            } else if self.pitch < -89.0 {
                self.pitch = -89.0;
            }
        }
        if response.contains_pointer() {
            let zoom_delta = ui.ctx().input(|i| i.zoom_delta());
            self.zoom -= (zoom_delta - 1.0) * 0.3;
            if self.zoom < 0.1 {
                self.zoom = 0.1;
            }
        }

        // Clone locals so we can move them into the paint callback:
        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.to_radians();
        let zoom = self.zoom;
        let program = self.render_ctx.program;
        let texture_array = self.render_ctx.texture_array;
        let model_viewer = self.render_ctx.model_viewer.clone();

        let callback = egui::PaintCallback {
            rect,
            callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                model_viewer.lock().paint(
                    painter.gl(),
                    rect.width(),
                    rect.height(),
                    yaw,
                    pitch,
                    zoom,
                    program,
                    texture_array,
                );
            })),
        };
        ui.painter().add(callback);
    }

    fn init_shader_program(gl: &Arc<glow::Context>) -> glow::Program {
        use glow::HasContext as _;

        let shader_version = if cfg!(target_arch = "wasm32") {
            "#version 300 es"
        } else {
            "#version 330"
        };

        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            let (vertex_shader_source, fragment_shader_source) = (
                r#"
                    #ifdef GL_NV_shader_noperspective_interpolation
                    #extension GL_NV_shader_noperspective_interpolation : require
                    #endif

                    uniform mat4 u_view;
                    uniform mat4 u_projection;

                    layout (location = 0) in vec3 a_position;
                    layout (location = 1) in uint a_hsl;
                    layout (location = 2) in float a_alpha;
                    layout (location = 3) in vec2 a_texcoord;
                    layout (location = 4) in uint a_texture_id;

                    flat out int v_hs;
                    #ifdef GL_NV_shader_noperspective_interpolation
                    noperspective centroid out float v_lightness;
                    #else
                    centroid out float v_lightness;
                    #endif
                    out float v_alpha;
                    out vec2 v_texcoord;
                    flat out int v_texture_id;

                    void main() {
                        int hsl = int(a_hsl);
                        v_hs = hsl & 0xff80;
                        v_lightness = float(hsl & 0x7f);
                        v_alpha = a_alpha;
                        v_texcoord = a_texcoord;
                        v_texture_id = int(a_texture_id);

                        gl_Position = u_projection * u_view * vec4(a_position, 1.0);
                    }
                "#,
                r#"
                    #ifdef GL_NV_shader_noperspective_interpolation
                    #extension GL_NV_shader_noperspective_interpolation : require
                    #endif

                    precision mediump float;

                    uniform highp sampler2DArray u_texture_array;

                    flat in int v_hs;
                    #ifdef GL_NV_shader_noperspective_interpolation
                    noperspective centroid in float v_lightness;
                    #else
                    centroid in float v_lightness;
                    #endif
                    in float v_alpha;
                    in vec2 v_texcoord;
                    flat in int v_texture_id;

                    out vec4 out_color;
                    
                    vec3 hslToRgb(int hsl, float brightness) {
                        const float onethird = 1.0 / 3.0;
                        const float twothird = 2.0 / 3.0;
                        const float rcpsixth = 6.0;

                        float hue = float(hsl >> 10) / 64.0 + 0.0078125;
                        float sat = float((hsl >> 7) & 0x7) / 8.0 + 0.0625;
                        float lum = float(hsl & 0x7f) / 128.0;

                        vec3 xt = vec3(
                            rcpsixth * (hue - twothird),
                            0.0,
                            rcpsixth * (1.0 - hue)
                        );

                        if (hue < twothird) {
                            xt.r = 0.0;
                            xt.g = rcpsixth * (twothird - hue);
                            xt.b = rcpsixth * (hue      - onethird);
                        }

                        if (hue < onethird) {
                            xt.r = rcpsixth * (onethird - hue);
                            xt.g = rcpsixth * hue;
                            xt.b = 0.0;
                        }

                        xt = min( xt, 1.0 );

                        float sat2   =  2.0 * sat;
                        float satinv =  1.0 - sat;
                        float luminv =  1.0 - lum;
                        float lum2m1 = (2.0 * lum) - 1.0;
                        vec3  ct     = (sat2 * xt) + satinv;

                        vec3 rgb;
                        if (lum >= 0.5)
                             rgb = (luminv * ct) + lum2m1;
                        else rgb =  lum    * ct;

                        return pow(rgb, vec3(brightness));
                    }

                    void main() {
                        out_color = vec4(hslToRgb(v_hs | int(v_lightness), 0.7), v_alpha);
                        if (v_texture_id > 0) {
                            out_color *= texture(u_texture_array, vec3(v_texcoord, float(v_texture_id - 1))).bgra;
                            if (out_color.a < 0.1) {
                                discard;
                            }
                        }
                    }
                "#,
            );

            let shader_sources = [
                (glow::VERTEX_SHADER, vertex_shader_source),
                (glow::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl
                        .create_shader(*shader_type)
                        .expect("Cannot create shader");
                    gl.shader_source(shader, &format!("{shader_version}\n{shader_source}"));
                    gl.compile_shader(shader);
                    assert!(
                        gl.get_shader_compile_status(shader),
                        "Failed to compile {shader_type}: {}",
                        gl.get_shader_info_log(shader)
                    );
                    gl.attach_shader(program, shader);
                    shader
                })
                .collect();

            gl.link_program(program);
            assert!(
                gl.get_program_link_status(program),
                "{}",
                gl.get_program_info_log(program)
            );

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            program
        }
    }

    fn init_texture_array(
        gl: &Arc<glow::Context>,
        texture_provider: &TextureProvider,
    ) -> glow::Texture {
        use glow::HasContext as _;

        let texture_size = 128;
        let texture_count = texture_provider.textures.len();

        unsafe {
            gl.active_texture(glow::TEXTURE0);
            let texture_array = gl.create_texture().expect("Cannot create texture");
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(texture_array));
            gl.tex_storage_3d(
                glow::TEXTURE_2D_ARRAY,
                1,
                glow::RGBA8,
                texture_size,
                texture_size,
                texture_count as i32,
            );

            for &texture_id in texture_provider.get_texture_ids().iter() {
                if let Some(pixels) = texture_provider.get_pixels_argb(
                    texture_id,
                    texture_size as u16,
                    texture_size as u16,
                    false,
                    0.7,
                ) {
                    gl.tex_sub_image_3d(
                        glow::TEXTURE_2D_ARRAY,
                        0,
                        0,
                        0,
                        texture_id as i32,
                        texture_size,
                        texture_size,
                        1,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(bytemuck::cast_slice(&pixels))),
                    );
                }
            }

            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D_ARRAY,
                glow::TEXTURE_WRAP_T,
                glow::REPEAT as i32,
            );

            texture_array
        }
    }
}

impl eframe::App for ModelViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                self.custom_painting(ui);
            });

        self.model_selector.show(
            ctx,
            &self.render_ctx,
            &self.model_js5,
            &self.texture_provider,
        );

        if let Some(id) = self.model_selector.selected_id.take() {
            self.selected_model_id = id;
        }

        if self.current_model_id != self.selected_model_id {
            if let Some(model_data) = self.model_js5.get_file(self.selected_model_id, 0) {
                let mut model_unlit = ModelUnlit::new();
                model_unlit.decode(&model_data);

                if model_unlit.version < 13 {
                    model_unlit.scale_log2(2);
                }

                let model = ModelLit::from_unlit(
                    &self.texture_provider,
                    &model_unlit,
                    ModelFlags::empty(),
                    64,
                    768,
                );

                self.render_ctx
                    .model_viewer
                    .lock()
                    .upload_model(&self.gl, model);
                self.current_model_id = self.selected_model_id;
            }
        }

        ctx.request_repaint(); // always repaint
    }
}

struct ModelSelectorWindow {
    gl: Arc<glow::Context>,
    start_time: f64,
    search_text: String,
    selected_id: Option<u32>,
    model_viewers: HashMap<usize, Arc<Mutex<ModelViewer>>>,
    active_preview_ids: HashSet<usize>,
    search_results: Vec<usize>,
}

impl ModelSelectorWindow {
    const YAW: f32 = 90.0;
    const PITCH: f32 = 30.0;

    const CONTAINER_WIDTH: f32 = 134.0;
    const CONTAINER_HEIGHT: f32 = 152.0;
    const CONTAINER_WIDTH_WITH_SPACING: f32 = Self::CONTAINER_WIDTH + 6.0;
    const CANVAS_SIZE: f32 = 128.0;

    fn new(gl: Arc<glow::Context>) -> Self {
        Self {
            gl,
            start_time: now(),
            search_text: "".to_owned(),
            selected_id: None,
            model_viewers: HashMap::new(),
            active_preview_ids: HashSet::new(),
            search_results: vec![],
        }
    }

    fn get_or_load_model(
        &mut self,
        model_js5: &Js5,
        texture_provider: &TextureProvider,
        id: usize,
    ) -> Option<Arc<Mutex<ModelViewer>>> {
        if let Some(model_viewer) = self.model_viewers.get(&id) {
            return Some(model_viewer.clone());
        }

        let mut model_unlit = ModelUnlit::from_js5(model_js5, id as u32, 0)?;

        if model_unlit.version < 13 {
            model_unlit.scale_log2(2);
        }

        let mut model =
            ModelLit::from_unlit(texture_provider, &model_unlit, ModelFlags::empty(), 64, 768);

        model = model.copy(ModelFlags::CHANGED_X | ModelFlags::CHANGED_Y | ModelFlags::CHANGED_Z);

        let (center_x, center_y, center_z) = model.get_center();
        model.translate(-center_x, -center_y, -center_z);

        let radius = model.get_xyz_radius() as f32 / 512.0 * 2.0;

        let model_viewer = Arc::new(Mutex::new(ModelViewer::new(radius)));
        model_viewer.lock().upload_model(&self.gl, model);

        self.model_viewers.insert(id, model_viewer.clone());

        Some(model_viewer)
    }

    fn show(
        &mut self,
        ctx: &egui::Context,
        render_ctx: &ModelRenderContext,
        model_js5: &Js5,
        texture_provider: &TextureProvider,
    ) {
        egui::Window::new("Model Selector")
            .resizable(true)
            .scroll(false)
            .show(ctx, |ui| {
                self.active_preview_ids.clear();

                self.ui(ui, render_ctx, model_js5, texture_provider);

                let mut to_remove = vec![];
                for id in self.model_viewers.keys() {
                    if !self.active_preview_ids.contains(id) {
                        to_remove.push(*id);
                    }
                }

                for id in to_remove {
                    let Some(model_viewer) = self.model_viewers.remove(&id) else {
                        continue;
                    };
                    model_viewer.lock().destroy(&self.gl);
                }
            });
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        render_ctx: &ModelRenderContext,
        model_js5: &Js5,
        texture_provider: &TextureProvider,
    ) {
        let search_response = ui.add(egui::TextEdit::singleline(&mut self.search_text).hint_text(
            format!(
                "Search models by id (0-{})...",
                model_js5.get_last_group_id()
            ),
        ));
        if search_response.changed() {
            self.search_results.clear();
            if !self.search_text.is_empty() {
                for index in 0..model_js5.get_group_count() as usize {
                    let id = model_js5.index.group_ids[index];
                    if id.to_string().contains(&self.search_text) {
                        self.search_results.push(id as usize);
                    }
                }
            }
            println!("Search text: {}", self.search_text);
        }

        let count = if self.search_results.is_empty() {
            model_js5.get_group_count() as usize
        } else {
            self.search_results.len()
        };

        ui.ctx().style_mut(|style| {
            style.interaction.selectable_labels = false;
            style.spacing.scroll = egui::style::ScrollStyle::solid()
        });

        ui.separator();

        let available_width = ui.available_width();

        let items_per_row = (available_width / Self::CONTAINER_WIDTH_WITH_SPACING).floor() as usize;
        let total_rows = count.div_ceil(items_per_row);

        let remaining_space = available_width
            - (items_per_row as f32 * Self::CONTAINER_WIDTH)
            - (items_per_row - 1) as f32 * 8.0;

        let padding = (remaining_space / 2.0).floor();

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_width(available_width)
            .show_rows(ui, Self::CONTAINER_HEIGHT, total_rows, |ui, row_range| {
                self.add_rows(
                    ui,
                    render_ctx,
                    model_js5,
                    texture_provider,
                    row_range,
                    count,
                    total_rows,
                    items_per_row,
                    padding,
                );
            });
    }

    fn add_rows(
        &mut self,
        ui: &mut egui::Ui,
        render_ctx: &ModelRenderContext,
        model_js5: &Js5,
        texture_provider: &TextureProvider,
        row_range: std::ops::Range<usize>,
        total_items: usize,
        total_rows: usize,
        items_per_row: usize,
        padding: f32,
    ) {
        for row in row_range {
            ui.horizontal(|ui| {
                ui.add_space(padding);
                let item_start = row * items_per_row;
                let item_end = (item_start + items_per_row).min(total_items);
                for index in item_start..item_end {
                    let id = if self.search_results.is_empty() {
                        model_js5.index.group_ids[index] as usize
                    } else {
                        self.search_results[index]
                    };
                    self.add_item(ui, render_ctx, model_js5, texture_provider, id);
                }
            });

            let is_last_row = row == total_rows - 1;
            if !is_last_row {
                ui.add_space(5.0);
            }
        }
    }

    fn add_item(
        &mut self,
        ui: &mut egui::Ui,
        render_ctx: &ModelRenderContext,
        model_js5: &Js5,
        texture_provider: &TextureProvider,
        id: usize,
    ) {
        self.active_preview_ids.insert(id);
        let response = ui
            .scope_builder(
                egui::UiBuilder::new()
                    // .id_salt("interactive_container")
                    .sense(egui::Sense::click()),
                |ui| {
                    ui.set_width(Self::CONTAINER_WIDTH);
                    let response = ui.response();
                    let visuals = ui.style().interact(&response);
                    let text_color = visuals.text_color();

                    let mut stroke = ui.style().visuals.window_stroke();
                    if response.hovered() {
                        stroke.color = egui::Color32::WHITE;
                    }

                    ui.vertical_centered(|ui| {
                        egui::Frame::dark_canvas(ui.style())
                            .stroke(stroke)
                            .show(ui, |ui| {
                                if let Some(model_viewer) =
                                    self.get_or_load_model(model_js5, texture_provider, id)
                                {
                                    let (rect, _response) = ui.allocate_exact_size(
                                        egui::Vec2::new(Self::CANVAS_SIZE, Self::CANVAS_SIZE),
                                        egui::Sense::empty(),
                                    );
                                    self.add_model(ui, render_ctx, rect, model_viewer);
                                } else {
                                    ui.set_width(128.0);
                                    ui.set_height(128.0);
                                    ui.centered_and_justified(|ui| {
                                        ui.spinner();
                                    });
                                }
                            });
                        ui.colored_label(text_color, id.to_string());
                        // ui.label("Long text that should wrap hopefully maybe");
                    });
                },
            )
            .response;

        if response.clicked() {
            self.selected_id = Some(id as u32);
        }
    }

    fn add_model(
        &mut self,
        ui: &mut egui::Ui,
        render_ctx: &ModelRenderContext,
        rect: egui::Rect,
        model_viewer: Arc<Mutex<ModelViewer>>,
    ) {
        let yaw = ((now() - self.start_time) / 1000.0 * 60.0).to_radians() as f32;

        // let yaw = Self::YAW.to_radians();
        let pitch = Self::PITCH.to_radians();
        let zoom = 1.0;
        let program = render_ctx.program;
        let texture_array = render_ctx.texture_array;

        let callback = egui::PaintCallback {
            rect,
            callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                model_viewer.lock().paint(
                    painter.gl(),
                    rect.width(),
                    rect.height(),
                    yaw,
                    pitch,
                    zoom,
                    program,
                    texture_array,
                );
            })),
        };
        ui.painter().add(callback);
    }
}

struct UploadedModel {
    triangle_count: i32,
    vertex_array: glow::VertexArray,
    position_buffer: glow::Buffer,
    colour_buffer: glow::Buffer,
    texcoord_buffer: glow::Buffer,
    texture_id_buffer: glow::Buffer,
}

impl UploadedModel {
    fn new(
        triangle_count: i32,
        vertex_array: glow::VertexArray,
        position_buffer: glow::Buffer,
        colour_buffer: glow::Buffer,
        texcoord_buffer: glow::Buffer,
        texture_id_buffer: glow::Buffer,
    ) -> Self {
        Self {
            triangle_count,
            vertex_array,
            position_buffer,
            colour_buffer,
            texcoord_buffer,
            texture_id_buffer,
        }
    }

    fn destroy(&self, gl: &glow::Context) {
        use glow::HasContext as _;
        unsafe {
            gl.delete_vertex_array(self.vertex_array);
            gl.delete_buffer(self.position_buffer);
            gl.delete_buffer(self.colour_buffer);
            gl.delete_buffer(self.texcoord_buffer);
            gl.delete_buffer(self.texture_id_buffer);
        }
    }
}

struct ModelViewer {
    radius: f32,
    uploaded_model: Option<UploadedModel>,
}

impl ModelViewer {
    fn new(radius: f32) -> Self {
        Self {
            radius,
            uploaded_model: None,
        }
    }

    fn upload_model(&mut self, gl: &glow::Context, model: ModelLit) {
        use glow::HasContext as _;

        if let Some(uploaded_model) = self.uploaded_model.take() {
            uploaded_model.destroy(gl);
        }

        let vertex_array = unsafe {
            gl.create_vertex_array()
                .expect("vertex array should be created")
        };
        let (triangle_colours_a, triangle_colours_b, triangle_colours_c) =
            model.calc_lit_colours(-50, -10, -50);
        // let (triangle_colours_a, triangle_colours_b, triangle_colours_c) = model.calc_lit_colours(-30, -50, -30);

        let mut vertex_x = vec![0; model.render_vertex_count as usize];
        let mut vertex_y = vec![0; model.render_vertex_count as usize];
        let mut vertex_z = vec![0; model.render_vertex_count as usize];
        for i in 0..model.used_vertex_count as usize {
            let v_start = model.vertex_unique_index[i] as usize;
            let v_end = model.vertex_unique_index[i + 1] as usize;
            for v in v_start..v_end {
                let mut pos = model.vertex_stream_pos[v] as usize;
                if pos == 0 {
                    break;
                }
                pos -= 1;
                vertex_x[pos] = model.vertex_x[i];
                vertex_y[pos] = model.vertex_y[i];
                vertex_z[pos] = model.vertex_z[i];
            }
        }

        let mut triangle_count = 0;

        let mut positions: Vec<f32> = Vec::with_capacity(model.triangle_count as usize * 3 * 3);
        let mut colours: Vec<u16> = Vec::with_capacity(model.triangle_count as usize * 3);
        let mut alphas: Vec<u8> = Vec::with_capacity(model.triangle_count as usize * 3);
        let mut texcoords: Vec<f32> = Vec::with_capacity(model.triangle_count as usize * 3 * 2);
        let mut texture_ids: Vec<u16> = Vec::with_capacity(model.triangle_count as usize * 3);
        for t in 0..model.render_triangle_count as usize {
            let a = model.triangle_render_a[t] as usize;
            let b = model.triangle_render_b[t] as usize;
            let c = model.triangle_render_c[t] as usize;

            let colour_a = triangle_colours_a[t];
            let mut colour_b = triangle_colours_b[t];
            let mut colour_c = triangle_colours_c[t];

            let alpha = 0xff - model.triangle_transparency[t];

            if colour_c == -2 {
                continue;
            }

            if colour_c == -1 {
                colour_c = colour_a;
                colour_b = colour_a;
            }

            let texture_id = (model.triangle_material[t] + 1) as u16;

            positions.push(vertex_x[a] as f32 / 512.0);
            positions.push(-vertex_y[a] as f32 / 512.0);
            positions.push(-vertex_z[a] as f32 / 512.0);

            positions.push(vertex_x[b] as f32 / 512.0);
            positions.push(-vertex_y[b] as f32 / 512.0);
            positions.push(-vertex_z[b] as f32 / 512.0);

            positions.push(vertex_x[c] as f32 / 512.0);
            positions.push(-vertex_y[c] as f32 / 512.0);
            positions.push(-vertex_z[c] as f32 / 512.0);

            // colours.push(model.triangle_colours[t]);
            // colours.push(model.triangle_colours[t]);
            // colours.push(model.triangle_colours[t]);
            colours.push(colour_a as u16);
            colours.push(colour_b as u16);
            colours.push(colour_c as u16);

            alphas.push(alpha);
            alphas.push(alpha);
            alphas.push(alpha);

            texcoords.push(model.texcoord_u[a]);
            texcoords.push(model.texcoord_v[a]);

            texcoords.push(model.texcoord_u[b]);
            texcoords.push(model.texcoord_v[b]);

            texcoords.push(model.texcoord_u[c]);
            texcoords.push(model.texcoord_v[c]);

            texture_ids.push(texture_id);
            texture_ids.push(texture_id);
            texture_ids.push(texture_id);

            triangle_count += 1;
        }

        unsafe {
            let position_buffer = gl
                .create_buffer()
                .expect("position buffer should be created");
            let colour_buffer = gl.create_buffer().expect("colour buffer should be created");
            let alpha_buffer = gl.create_buffer().expect("alpha buffer should be created");
            let texcoord_buffer = gl
                .create_buffer()
                .expect("texcoord buffer should be created");
            let texture_id_buffer = gl
                .create_buffer()
                .expect("texture id buffer should be created");

            gl.bind_vertex_array(Some(vertex_array));

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(position_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&positions),
                glow::STATIC_DRAW,
            );

            gl.vertex_attrib_pointer_f32(
                0,
                3,
                glow::FLOAT,
                false,
                std::mem::size_of::<f32>() as i32 * 3, /* + std::mem::size_of::<u16>() as i32*/
                0,
            );

            gl.enable_vertex_attrib_array(0);

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(colour_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&colours),
                glow::STATIC_DRAW,
            );

            gl.vertex_attrib_pointer_i32(
                1,
                1,
                glow::UNSIGNED_SHORT,
                std::mem::size_of::<u16>() as i32,
                0,
            );

            gl.enable_vertex_attrib_array(1);

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(alpha_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&alphas),
                glow::STATIC_DRAW,
            );

            gl.vertex_attrib_pointer_f32(
                2,
                1,
                glow::UNSIGNED_BYTE,
                true,
                std::mem::size_of::<u8>() as i32,
                0,
            );

            gl.enable_vertex_attrib_array(2);

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(texcoord_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&texcoords),
                glow::STATIC_DRAW,
            );

            gl.vertex_attrib_pointer_f32(
                3,
                2,
                glow::FLOAT,
                false,
                std::mem::size_of::<f32>() as i32 * 2,
                0,
            );

            gl.enable_vertex_attrib_array(3);

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(texture_id_buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&texture_ids),
                glow::STATIC_DRAW,
            );

            gl.vertex_attrib_pointer_i32(
                4,
                1,
                glow::UNSIGNED_SHORT,
                std::mem::size_of::<u16>() as i32,
                0,
            );

            gl.enable_vertex_attrib_array(4);

            self.uploaded_model = Some(UploadedModel::new(
                triangle_count,
                vertex_array,
                position_buffer,
                colour_buffer,
                texcoord_buffer,
                texture_id_buffer,
            ));
        }
    }

    fn destroy(&mut self, gl: &glow::Context) {
        if let Some(uploaded_model) = self.uploaded_model.take() {
            uploaded_model.destroy(gl);
        }
    }

    fn paint(
        &self,
        gl: &glow::Context,
        width: f32,
        height: f32,
        yaw: f32,
        pitch: f32,
        zoom: f32,
        program: glow::Program,
        texture_array: glow::Texture,
    ) {
        use glow::HasContext as _;

        let aspect = width / height;
        let field_of_view = 60f32;

        let radius: f32 = self.radius * zoom;

        let camera_front = glm::normalize(&glm::vec3(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        ));

        let view = glm::look_at(
            &(camera_front * radius),
            &glm::vec3(0.0, 0.0, 0.0),
            &glm::vec3(0.0, 1.0, 0.0),
        );

        let projection = glm::perspective(aspect, field_of_view.to_radians(), 0.1f32, 100.0f32);

        unsafe {
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
            gl.enable(glow::DEPTH_TEST);
            // gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            if let Some(uploaded_model) = &self.uploaded_model {
                gl.use_program(Some(program));
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(texture_array));
                gl.uniform_matrix_4_f32_slice(
                    gl.get_uniform_location(program, "u_view").as_ref(),
                    false,
                    view.as_slice(),
                );
                gl.uniform_matrix_4_f32_slice(
                    gl.get_uniform_location(program, "u_projection").as_ref(),
                    false,
                    projection.as_slice(),
                );
                gl.uniform_1_i32(
                    gl.get_uniform_location(program, "u_texture_array").as_ref(),
                    0,
                );

                gl.bind_vertex_array(Some(uploaded_model.vertex_array));
                gl.draw_arrays(glow::TRIANGLES, 0, uploaded_model.triangle_count * 3);
            }
        }
    }
}
