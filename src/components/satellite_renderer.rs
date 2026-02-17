#![allow(clippy::all)]
//! Satellite 3D rendering module
//! Handles WebGL rendering of Earth and satellites

use web_sys::{WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlShader};
use crate::components::satellite_calculations::SatellitePosition;

pub struct SatelliteRenderer {
    gl: WebGl2RenderingContext,
    program: Option<WebGlProgram>,
    earth_vertex_buffer: Option<WebGlBuffer>,
    earth_index_buffer: Option<WebGlBuffer>,
    earth_index_count: i32,
    equator_vertex_buffer: Option<WebGlBuffer>,
    equator_vertex_count: i32,
    camera_angle_horizontal: f32,
    camera_angle_vertical: f32,
    camera_distance: f32,
    auto_rotate: bool,
    satellite_positions: Vec<SatellitePosition>,
    satellite_vertex_buffer: Option<WebGlBuffer>,
}

impl SatelliteRenderer {
    pub fn new(gl: WebGl2RenderingContext) -> Result<Self, String> {
        Ok(Self {
            gl,
            program: None,
            earth_vertex_buffer: None,
            earth_index_buffer: None,
            earth_index_count: 0,
            equator_vertex_buffer: None,
            equator_vertex_count: 0,
            camera_angle_horizontal: 0.0,
            camera_angle_vertical: 0.5,
            camera_distance: 4.0,
            auto_rotate: true,
            satellite_positions: Vec::new(),
            satellite_vertex_buffer: None,
        })
    }

    /// Adjust camera zoom (positive = zoom in, negative = zoom out)
    pub fn adjust_zoom(&mut self, delta: f32) {
        // Make zoom speed proportional to current distance (faster when far away)
        let zoom_factor = self.camera_distance * 0.1;
        self.camera_distance = (self.camera_distance - delta * zoom_factor).clamp(1.5, 20.0);
    }

    /// Rotate camera based on mouse drag
    pub fn rotate_camera(&mut self, delta_x: f32, delta_y: f32) {
        self.auto_rotate = false; // Disable auto-rotation when user interacts
        self.camera_angle_horizontal += delta_x * 0.01;
        self.camera_angle_vertical = (self.camera_angle_vertical - delta_y * 0.01).clamp(-1.5, 1.5);
    }

    /// Set camera to a preset view
    pub fn set_preset_view(&mut self, preset: &str) {
        self.auto_rotate = false;
        match preset {
            "equator" => {
                // Side view of equator
                self.camera_angle_horizontal = 0.0;
                self.camera_angle_vertical = 0.0;
            }
            "north" => {
                // Top-down view of North Pole
                self.camera_angle_horizontal = 0.0;
                self.camera_angle_vertical = std::f32::consts::PI / 2.0 - 0.1; // Almost straight down
            }
            "south" => {
                // Bottom-up view of South Pole
                self.camera_angle_horizontal = 0.0;
                self.camera_angle_vertical = -(std::f32::consts::PI / 2.0 - 0.1); // Almost straight up
            }
            "oblique" => {
                // Default oblique view
                self.camera_angle_horizontal = 0.0;
                self.camera_angle_vertical = 0.5;
            }
            _ => {}
        }
    }

    /// Update satellite positions
    pub fn update_satellites(&mut self, positions: Vec<SatellitePosition>) {
        self.satellite_positions = positions;

        // Create/update satellite vertex buffer
        if !self.satellite_positions.is_empty() {
            // Convert positions to vertex data (position + color)
            let mut vertices = Vec::new();
            for pos in &self.satellite_positions {
                // Position
                vertices.push(pos.x);
                vertices.push(pos.y);
                vertices.push(pos.z);
                // Color (bright white/yellow for satellites)
                vertices.push(1.0);
                vertices.push(1.0);
                vertices.push(0.8);
            }

            if self.satellite_vertex_buffer.is_none() {
                self.satellite_vertex_buffer = self.gl.create_buffer();
            }

            if let Some(buffer) = &self.satellite_vertex_buffer {
                self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(buffer));

                unsafe {
                    let vertices_array = js_sys::Float32Array::view(&vertices);
                    self.gl.buffer_data_with_array_buffer_view(
                        WebGl2RenderingContext::ARRAY_BUFFER,
                        &vertices_array,
                        WebGl2RenderingContext::DYNAMIC_DRAW,
                    );
                }
            }
        }
    }

    pub fn initialize(&mut self) -> Result<(), String> {
        // Create shader program
        let vert_shader = Self::compile_shader(
            &self.gl,
            WebGl2RenderingContext::VERTEX_SHADER,
            VERTEX_SHADER_SOURCE,
        )?;

        let frag_shader = Self::compile_shader(
            &self.gl,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            FRAGMENT_SHADER_SOURCE,
        )?;

        let program = Self::link_program(&self.gl, &vert_shader, &frag_shader)?;
        self.program = Some(program);

        // Set up WebGL state
        self.gl.enable(WebGl2RenderingContext::DEPTH_TEST);
        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);

        // Create Earth sphere geometry
        self.create_earth_sphere()?;

        // Create equator line
        self.create_equator_line()?;

        Ok(())
    }

    fn create_earth_sphere(&mut self) -> Result<(), String> {
        // Generate sphere geometry
        let (vertices, indices) = Self::generate_sphere(1.0, 32, 32);
        self.earth_index_count = indices.len() as i32;

        // Create vertex buffer
        let vertex_buffer = self.gl.create_buffer()
            .ok_or("Failed to create vertex buffer")?;
        self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vertex_buffer));

        unsafe {
            let vertices_array = js_sys::Float32Array::view(&vertices);
            self.gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER,
                &vertices_array,
                WebGl2RenderingContext::STATIC_DRAW,
            );
        }

        self.earth_vertex_buffer = Some(vertex_buffer);

        // Create index buffer
        let index_buffer = self.gl.create_buffer()
            .ok_or("Failed to create index buffer")?;
        self.gl.bind_buffer(WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER, Some(&index_buffer));

        unsafe {
            let indices_array = js_sys::Uint16Array::view(&indices);
            self.gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
                &indices_array,
                WebGl2RenderingContext::STATIC_DRAW,
            );
        }

        self.earth_index_buffer = Some(index_buffer);

        Ok(())
    }

    fn create_equator_line(&mut self) -> Result<(), String> {
        // Generate equator circle at y=0, slightly above Earth surface
        let radius = 1.01; // Slightly larger than Earth radius
        let segments = 128;
        let mut vertices = Vec::new();

        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
            let x = radius * angle.cos();
            let z = radius * angle.sin();

            // Position
            vertices.push(x);
            vertices.push(0.0); // y = 0 for equator
            vertices.push(z);

            // Color (bright yellow/gold for visibility)
            vertices.push(1.0); // R
            vertices.push(0.9); // G
            vertices.push(0.2); // B
        }

        self.equator_vertex_count = (segments + 1) as i32;

        // Create vertex buffer
        let vertex_buffer = self.gl.create_buffer()
            .ok_or("Failed to create equator vertex buffer")?;
        self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vertex_buffer));

        unsafe {
            let vertices_array = js_sys::Float32Array::view(&vertices);
            self.gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER,
                &vertices_array,
                WebGl2RenderingContext::STATIC_DRAW,
            );
        }

        self.equator_vertex_buffer = Some(vertex_buffer);

        Ok(())
    }

    fn generate_sphere(radius: f32, latitude_bands: u32, longitude_bands: u32) -> (Vec<f32>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Generate vertices
        for lat in 0..=latitude_bands {
            let theta = lat as f32 * std::f32::consts::PI / latitude_bands as f32;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            for long in 0..=longitude_bands {
                let phi = long as f32 * 2.0 * std::f32::consts::PI / longitude_bands as f32;
                let sin_phi = phi.sin();
                let cos_phi = phi.cos();

                let x = cos_phi * sin_theta;
                let y = cos_theta;
                let z = sin_phi * sin_theta;

                // Position
                vertices.push(radius * x);
                vertices.push(radius * y);
                vertices.push(radius * z);

                // Color (blue-green for Earth)
                vertices.push(0.2 + 0.3 * (y + 1.0) / 2.0); // R
                vertices.push(0.4 + 0.3 * (y + 1.0) / 2.0); // G
                vertices.push(0.8); // B
            }
        }

        // Generate indices
        for lat in 0..latitude_bands {
            for long in 0..longitude_bands {
                let first = (lat * (longitude_bands + 1) + long) as u16;
                let second = first + longitude_bands as u16 + 1;

                indices.push(first);
                indices.push(second);
                indices.push(first + 1);

                indices.push(second);
                indices.push(second + 1);
                indices.push(first + 1);
            }
        }

        (vertices, indices)
    }

    pub fn render(&mut self) {
        self.gl.clear(
            WebGl2RenderingContext::COLOR_BUFFER_BIT | WebGl2RenderingContext::DEPTH_BUFFER_BIT,
        );

        if let Some(program) = &self.program {
            self.gl.use_program(Some(program));

            // Update camera orbit angle (slow rotation around Earth) if auto-rotate is enabled
            if self.auto_rotate {
                self.camera_angle_horizontal += 0.002;
            }

            // Set up matrices - orbit camera around Earth
            let camera_x = self.camera_distance * self.camera_angle_horizontal.cos() * self.camera_angle_vertical.cos();
            let camera_z = self.camera_distance * self.camera_angle_horizontal.sin() * self.camera_angle_vertical.cos();
            let camera_y = self.camera_distance * self.camera_angle_vertical.sin();

            let projection = Self::perspective_matrix(45.0, 1200.0 / 600.0, 0.1, 100.0);
            let view = Self::look_at([camera_x, camera_y, camera_z], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
            let model = [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ]; // Identity matrix - Earth stays fixed
            let model_view = Self::multiply_matrices(&view, &model);

            // Set uniforms
            let projection_loc = self.gl.get_uniform_location(program, "uProjectionMatrix");
            self.gl.uniform_matrix4fv_with_f32_array(projection_loc.as_ref(), false, &projection);

            let model_view_loc = self.gl.get_uniform_location(program, "uModelViewMatrix");
            self.gl.uniform_matrix4fv_with_f32_array(model_view_loc.as_ref(), false, &model_view);

            // Draw Earth
            if let (Some(vertex_buffer), Some(index_buffer)) = (&self.earth_vertex_buffer, &self.earth_index_buffer) {
                self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(vertex_buffer));
                self.gl.bind_buffer(WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER, Some(index_buffer));

                let position_loc = self.gl.get_attrib_location(program, "position") as u32;
                let color_loc = self.gl.get_attrib_location(program, "color") as u32;

                let stride = 6 * 4; // 6 floats * 4 bytes
                self.gl.vertex_attrib_pointer_with_i32(position_loc, 3, WebGl2RenderingContext::FLOAT, false, stride, 0);
                self.gl.enable_vertex_attrib_array(position_loc);

                self.gl.vertex_attrib_pointer_with_i32(color_loc, 3, WebGl2RenderingContext::FLOAT, false, stride, 3 * 4);
                self.gl.enable_vertex_attrib_array(color_loc);

                self.gl.draw_elements_with_i32(
                    WebGl2RenderingContext::TRIANGLES,
                    self.earth_index_count,
                    WebGl2RenderingContext::UNSIGNED_SHORT,
                    0,
                );
            }

            // Draw equator line
            if let Some(equator_buffer) = &self.equator_vertex_buffer {
                self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(equator_buffer));

                let position_loc = self.gl.get_attrib_location(program, "position") as u32;
                let color_loc = self.gl.get_attrib_location(program, "color") as u32;

                let stride = 6 * 4; // 6 floats * 4 bytes
                self.gl.vertex_attrib_pointer_with_i32(position_loc, 3, WebGl2RenderingContext::FLOAT, false, stride, 0);
                self.gl.enable_vertex_attrib_array(position_loc);

                self.gl.vertex_attrib_pointer_with_i32(color_loc, 3, WebGl2RenderingContext::FLOAT, false, stride, 3 * 4);
                self.gl.enable_vertex_attrib_array(color_loc);

                self.gl.line_width(2.0);
                self.gl.draw_arrays(
                    WebGl2RenderingContext::LINE_STRIP,
                    0,
                    self.equator_vertex_count,
                );
            }

            // Draw satellites
            if let Some(sat_buffer) = &self.satellite_vertex_buffer {
                if !self.satellite_positions.is_empty() {
                    self.gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(sat_buffer));

                    let position_loc = self.gl.get_attrib_location(program, "position") as u32;
                    let color_loc = self.gl.get_attrib_location(program, "color") as u32;

                    let stride = 6 * 4; // 6 floats * 4 bytes
                    self.gl.vertex_attrib_pointer_with_i32(position_loc, 3, WebGl2RenderingContext::FLOAT, false, stride, 0);
                    self.gl.enable_vertex_attrib_array(position_loc);

                    self.gl.vertex_attrib_pointer_with_i32(color_loc, 3, WebGl2RenderingContext::FLOAT, false, stride, 3 * 4);
                    self.gl.enable_vertex_attrib_array(color_loc);

                    self.gl.draw_arrays(
                        WebGl2RenderingContext::POINTS,
                        0,
                        self.satellite_positions.len() as i32,
                    );
                }
            }
        }
    }

    // Matrix math helpers
    fn perspective_matrix(fov_degrees: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
        let f = 1.0 / (fov_degrees * std::f32::consts::PI / 360.0).tan();
        let nf = 1.0 / (near - far);

        [
            f / aspect, 0.0, 0.0, 0.0,
            0.0, f, 0.0, 0.0,
            0.0, 0.0, (far + near) * nf, -1.0,
            0.0, 0.0, 2.0 * far * near * nf, 0.0,
        ]
    }

    fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [f32; 16] {
        let z = [
            eye[0] - center[0],
            eye[1] - center[1],
            eye[2] - center[2],
        ];
        let z_len = (z[0] * z[0] + z[1] * z[1] + z[2] * z[2]).sqrt();
        let z = [z[0] / z_len, z[1] / z_len, z[2] / z_len];

        let x = [
            up[1] * z[2] - up[2] * z[1],
            up[2] * z[0] - up[0] * z[2],
            up[0] * z[1] - up[1] * z[0],
        ];
        let x_len = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
        let x = [x[0] / x_len, x[1] / x_len, x[2] / x_len];

        let y = [
            z[1] * x[2] - z[2] * x[1],
            z[2] * x[0] - z[0] * x[2],
            z[0] * x[1] - z[1] * x[0],
        ];

        [
            x[0], y[0], z[0], 0.0,
            x[1], y[1], z[1], 0.0,
            x[2], y[2], z[2], 0.0,
            -(x[0] * eye[0] + x[1] * eye[1] + x[2] * eye[2]),
            -(y[0] * eye[0] + y[1] * eye[1] + y[2] * eye[2]),
            -(z[0] * eye[0] + z[1] * eye[1] + z[2] * eye[2]),
            1.0,
        ]
    }


    fn multiply_matrices(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
        let mut result = [0.0; 16];
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result[i * 4 + j] += a[i * 4 + k] * b[k * 4 + j];
                }
            }
        }
        result
    }

    fn compile_shader(
        gl: &WebGl2RenderingContext,
        shader_type: u32,
        source: &str,
    ) -> Result<WebGlShader, String> {
        let shader = gl
            .create_shader(shader_type)
            .ok_or_else(|| String::from("Unable to create shader object"))?;

        gl.shader_source(&shader, source);
        gl.compile_shader(&shader);

        if gl
            .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            Ok(shader)
        } else {
            Err(gl
                .get_shader_info_log(&shader)
                .unwrap_or_else(|| String::from("Unknown error creating shader")))
        }
    }

    fn link_program(
        gl: &WebGl2RenderingContext,
        vert_shader: &WebGlShader,
        frag_shader: &WebGlShader,
    ) -> Result<WebGlProgram, String> {
        let program = gl
            .create_program()
            .ok_or_else(|| String::from("Unable to create shader program"))?;

        gl.attach_shader(&program, vert_shader);
        gl.attach_shader(&program, frag_shader);
        gl.link_program(&program);

        if gl
            .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
            .as_bool()
            .unwrap_or(false)
        {
            Ok(program)
        } else {
            Err(gl
                .get_program_info_log(&program)
                .unwrap_or_else(|| String::from("Unknown error creating program")))
        }
    }
}

// Basic vertex shader
const VERTEX_SHADER_SOURCE: &str = r#"#version 300 es
in vec3 position;
in vec3 color;

out vec3 vColor;

uniform mat4 uModelViewMatrix;
uniform mat4 uProjectionMatrix;

void main() {
    vColor = color;
    gl_Position = uProjectionMatrix * uModelViewMatrix * vec4(position, 1.0);
    gl_PointSize = 2.0;
}
"#;

// Basic fragment shader
const FRAGMENT_SHADER_SOURCE: &str = r#"#version 300 es
precision highp float;

in vec3 vColor;
out vec4 fragColor;

void main() {
    fragColor = vec4(vColor, 1.0);
}
"#;
