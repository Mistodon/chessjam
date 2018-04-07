#![allow(unknown_lints)] // TODO(claire): Can go when clippy lints go.

extern crate chessjam;

#[macro_use]
extern crate static_assets;
#[macro_use]
extern crate glium;

extern crate adequate_math;
extern crate wavefront_obj;

mod graphics;
mod input;

use adequate_math::*;
use glium::Display;
use glium::glutin::EventsLoop;

use input::*;


#[derive(Debug)]
pub struct Piece {
    pub position: Vec2<i32>,
    pub color: ChessColor,
    pub piece_type: PieceType,
}

#[derive(Debug)]
pub enum ChessColor {
    Black,
    White,
}

#[derive(Debug)]
pub enum PieceType {
    Pawn,
}


fn main() {
    use glium::glutin::{Api, ContextBuilder, GlProfile, GlRequest, WindowBuilder};

    let mut events_loop = EventsLoop::new();

    let window = WindowBuilder::new()
        .with_dimensions(1280, 720)
        .with_title("Purchess");

    let context = ContextBuilder::new()
        .with_depth_buffer(24)
        .with_gl_profile(GlProfile::Core)
        .with_gl(GlRequest::Specific(Api::OpenGl, (4, 0)))
        .with_multisampling(4)
        .with_vsync(true);

    let display = &Display::new(window, context, &events_loop).unwrap();

    loop {
        let rerun = run_game(display, &mut events_loop);

        if !rerun {
            break;
        }
    }
}


fn run_game(display: &Display, events_loop: &mut EventsLoop) -> bool {
    use std::time::Instant;

    let config = chessjam::config::load_config();

    let model_shader = graphics::create_shader(
        display,
        asset_str!("assets/shaders/model.glsl").as_ref(),
    );

    let shadow_shader = graphics::create_shader(
        display,
        asset_str!("assets/shaders/shadow.glsl").as_ref(),
    );

    let cube_mesh = graphics::create_obj_mesh(
        display,
        asset_str!("assets/meshes/tile.obj").as_ref(),
    );

    let pawn_mesh = graphics::create_obj_mesh(
        display,
        asset_str!("assets/meshes/pawn.obj").as_ref(),
    );

    let mut frame_time = Instant::now();
    let mut keyboard = Keyboard::default();
    let mut mouse = Mouse::default();


    const TARGET_ASPECT: f32 = 16.0 / 9.0;
    const CAMERA_NEAR_PLANE: f32 = 0.1;
    let camera_fov = consts::TAU32 * config.camera.fov as f32;
    let projection_matrix = matrix::perspective_projection(
        TARGET_ASPECT,
        camera_fov,
        CAMERA_NEAR_PLANE,
        100.0,
    );

    let camera_position = vec3(
        0.0,
        config.camera.height,
        -config.camera.distance,
    ).as_f32();
    let camera_focus = vec3(0.0, 0.0, 0.0);
    let camera_direction = camera_focus - camera_position;

    let view_matrix = {
        let orientation = matrix::look_rotation(camera_direction, vec3(0.0, 1.0, 0.0));
        let translation = Mat4::translation(-camera_position);
        orientation.transpose() * translation
    };

    let view_projection_matrix = projection_matrix * view_matrix;

    let shadow_direction = Vec4::from_slice(&config.light.key_dir)
        .norm()
        .as_f32();
    let light_direction_matrix: Mat4<f32> = {
        let key = shadow_direction;
        let fill = Vec4::from_slice(&config.light.fill_dir)
            .norm()
            .as_f32();
        let back = Vec4::from_slice(&config.light.back_dir)
            .norm()
            .as_f32();

        Mat4([key.0, fill.0, back.0, [0.0, 0.0, 0.0, 1.0]]).transpose()
    };

    let light_color_matrix: Mat4<f32> = Mat4([
        Vec4::from_slice(&config.light.key_color)
            .as_f32()
            .0,
        Vec4::from_slice(&config.light.fill_color)
            .as_f32()
            .0,
        Vec4::from_slice(&config.light.back_color)
            .as_f32()
            .0,
        Vec4::from_slice(&config.light.amb_color)
            .as_f32()
            .0,
    ]);

    let shadow_color_matrix: Mat4<f32> = Mat4([
        Vec4::from_slice(&config.shadow.key_color)
            .as_f32()
            .0,
        Vec4::from_slice(&config.shadow.fill_color)
            .as_f32()
            .0,
        Vec4::from_slice(&config.shadow.back_color)
            .as_f32()
            .0,
        Vec4::from_slice(&config.shadow.amb_color)
            .as_f32()
            .0,
    ]);

    let mut pieces = {
        let mut pieces = Vec::new();
        for x in 0..8 {
            pieces.push(Piece {
                position: vec2(x, 1),
                color: ChessColor::White,
                piece_type: PieceType::Pawn,
            });
            pieces.push(Piece {
                position: vec2(x, 6),
                color: ChessColor::Black,
                piece_type: PieceType::Pawn,
            });
        }
        pieces
    };
    let mut selected_piece_index: Option<usize> = None;

    loop {
        let (_dt, now) = chessjam::delta_time(frame_time);
        frame_time = now;

        // handle_events
        let mut closed = false;
        {
            use glium::glutin::{ElementState, Event, WindowEvent};

            let mut keyboard = keyboard.begin_frame_input();
            let mut mouse = mouse.begin_frame_input();

            #[allow(single_match)]
            events_loop.poll_events(|event| match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::KeyboardInput { input, .. } => {
                        let pressed = input.state == ElementState::Pressed;

                        if let Some(key) = input.virtual_keycode {
                            if pressed {
                                keyboard.press(key, input.modifiers);
                            }
                            else {
                                keyboard.release(key, input.modifiers);
                            }
                        }
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        let pressed = state == ElementState::Pressed;
                        if pressed {
                            mouse.press(button);
                        }
                        else {
                            mouse.release(button);
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let (x, y) = position;
                        mouse.move_cursor_to(x, y);
                    }
                    WindowEvent::Closed => closed = true,
                    _ => (),
                },
                _ => (),
            });
        }

        if closed || keyboard.pressed(Key::Escape) {
            return false;
        }
        if keyboard.pressed(Key::R) && keyboard.modifiers.logo {
            return true;
        }

        let tile_cursor = {
            let camera_forward = camera_direction.norm();
            let camera_right = vec3(0.0, 1.0, 0.0).cross(camera_forward).norm();
            let camera_up = camera_forward.cross(camera_right);

            let (w, h) = display.get_framebuffer_dimensions();
            let mouse_pos = (Vec2(mouse.position()) / vec2(w, h).as_f64()).as_f32();
            let (mx, my) = ((mouse_pos - vec2(0.5, 0.5)) * vec2(2.0, -2.0)).as_tuple();
            let near_plane_half_height = (camera_fov / 2.0).tan() * CAMERA_NEAR_PLANE;
            let near_plane_half_width = near_plane_half_height * TARGET_ASPECT;

            let mouse_ray = {
                (camera_right * mx * near_plane_half_width)
                    + (camera_up * my * near_plane_half_height)
                    + (camera_forward * CAMERA_NEAR_PLANE)
            };

            let t = -(camera_position.0[1] / mouse_ray.0[1]);
            let hit = camera_position + mouse_ray * t;

            chessjam::world_to_grid(hit)
        };

        // update
        {
            fn piece_at(position: Vec2<i32>, pieces: &[Piece]) -> Option<usize> {
                let mut result = None;
                for (index, piece) in pieces.iter().enumerate() {
                    if piece.position == position {
                        result = Some(index);
                    }
                }
                result
            }

            if mouse.pressed(Button::Left) {
                match selected_piece_index {
                    None => {
                        selected_piece_index = piece_at(tile_cursor, &pieces);
                    }
                    Some(index) => {
                        if piece_at(tile_cursor, &pieces).is_none() {
                            pieces[index].position = tile_cursor;
                            selected_piece_index = None;
                        }
                    }
                }
            }
        }

        // render
        {
            use glium::{
                Blend, BackfaceCullingMode, Depth, DepthTest, DrawParameters, Rect,
                Surface, draw_parameters::{Stencil, StencilOperation, StencilTest},
            };
            use graphics::Mesh;

            struct RenderCommand<'a> {
                position: Vec3<f32>,
                mesh: &'a Mesh,
                color: Vec4<f32>,
            }

            let mut render_buffer = Vec::with_capacity(100);

            // Add some chessboard squares
            for y in 0..8 {
                for x in 0..8 {
                    let position = vec3(-3.5, 0.0, -3.5) + vec3(x, 0, y).as_f32();
                    let color = match (x + y) % 2 {
                        0 => Vec4::from_slice(&config.colors.black).as_f32(),
                        _ => Vec4::from_slice(&config.colors.white).as_f32(),
                    };
                    render_buffer.push(RenderCommand {
                        position,
                        mesh: &cube_mesh,
                        color,
                    });
                }
            }

            // Add some chess pieces
            for piece in &pieces {
                let color = match piece.color {
                    ChessColor::Black => Vec4::from_slice(&config.colors.grey).as_f32(),
                    ChessColor::White => Vec4::from_slice(&config.colors.white).as_f32(),
                };

                let mesh = match piece.piece_type {
                    PieceType::Pawn => &pawn_mesh,
                };

                render_buffer.push(RenderCommand {
                    position: chessjam::grid_to_world(piece.position),
                    mesh,
                    color,
                });
            }

            let mut frame = display.draw();
            frame.clear_all_srgb((0.0, 0.0, 0.0, 1.0), 1.0, 0);

            let viewport = {
                let (left, bottom, width, height) = chessjam::viewport_rect(
                    display.get_framebuffer_dimensions(),
                    TARGET_ASPECT,
                );
                Rect {
                    left,
                    bottom,
                    width,
                    height,
                }
            };

            let clear_color = Vec4::from_slice(&config.colors.sky)
                .as_f32()
                .as_tuple();
            frame.clear(
                Some(&viewport),
                Some(clear_color),
                true,
                None,
                None,
            );

            let draw_params = DrawParameters {
                depth: Depth {
                    test: DepthTest::IfLess,
                    write: true,
                    ..Default::default()
                },
                backface_culling: BackfaceCullingMode::CullClockwise,
                viewport: Some(viewport),
                ..Default::default()
            };


            // Render all objects as if in shadow
            for command in &render_buffer {
                let model_matrix = Mat4::translation(command.position);
                let normal_matrix = Mat3::<f32>::identity();
                let mvp_matrix = view_projection_matrix * model_matrix;

                frame
                    .draw(
                        &command.mesh.vertices,
                        &command.mesh.indices,
                        &model_shader,
                        &uniform!{
                            transform: mvp_matrix.0,
                            normal_matrix: normal_matrix.0,
                            light_direction_matrix: light_direction_matrix.0,
                            light_color_matrix: shadow_color_matrix.0,
                            albedo: command.color.0,
                        },
                        &draw_params,
                    )
                    .unwrap();
            }


            let shadow_front_draw_params = DrawParameters {
                depth: Depth {
                    test: DepthTest::IfLess,
                    write: false,
                    ..Default::default()
                },
                color_mask: (false, false, false, false),
                stencil: Stencil {
                    depth_pass_operation_clockwise: StencilOperation::Increment,
                    ..Default::default()
                },
                backface_culling: BackfaceCullingMode::CullCounterClockwise,
                viewport: Some(viewport),
                ..Default::default()
            };

            let shadow_back_draw_params = DrawParameters {
                depth: Depth {
                    test: DepthTest::IfLess,
                    write: false,
                    ..Default::default()
                },
                color_mask: (false, false, false, false),
                stencil: Stencil {
                    depth_pass_operation_counter_clockwise:
                        StencilOperation::Decrement,
                    ..Default::default()
                },
                backface_culling: BackfaceCullingMode::CullClockwise,
                viewport: Some(viewport),
                ..Default::default()
            };

            // Render shadows: front then back
            for draw_params in &[
                shadow_front_draw_params,
                shadow_back_draw_params,
            ] {
                for command in &render_buffer {
                    let model_matrix = Mat4::translation(command.position);
                    let mvp_matrix = view_projection_matrix * model_matrix;
                    let model_space_shadow_direction = shadow_direction.retract();


                    frame
                        .draw(
                            &command.mesh.shadow_vertices,
                            &command.mesh.shadow_indices,
                            &shadow_shader,
                            &uniform!{
                                transform: mvp_matrix.0,
                                model_space_shadow_direction:
                                    model_space_shadow_direction.0,
                            },
                            draw_params,
                        )
                        .unwrap();
                }
            }

            // Render objects fully lit outside shadow volumes
            for command in &render_buffer {
                let model_matrix = Mat4::translation(command.position);
                let normal_matrix = Mat3::<f32>::identity();
                let mvp_matrix = view_projection_matrix * model_matrix;

                let fully_lit_draw_params = DrawParameters {
                    depth: Depth {
                        test: DepthTest::IfLessOrEqual,
                        write: false,
                        ..Default::default()
                    },
                    stencil: Stencil {
                        test_counter_clockwise: StencilTest::IfEqual { mask: !0 },
                        reference_value_counter_clockwise: 0,
                        ..Default::default()
                    },
                    backface_culling: BackfaceCullingMode::CullClockwise,
                    viewport: Some(viewport),
                    ..Default::default()
                };

                frame
                    .draw(
                        &command.mesh.vertices,
                        &command.mesh.indices,
                        &model_shader,
                        &uniform!{
                            transform: mvp_matrix.0,
                            normal_matrix: normal_matrix.0,
                            light_direction_matrix: light_direction_matrix.0,
                            light_color_matrix: light_color_matrix.0,
                            albedo: command.color.0,
                        },
                        &fully_lit_draw_params,
                    )
                    .unwrap();
            }

            // Render cursor
            {
                let cursor_draw_params = DrawParameters {
                    depth: Depth {
                        test: DepthTest::IfLess,
                        write: false,
                        ..Default::default()
                    },
                    blend: Blend::alpha_blending(),
                    backface_culling: BackfaceCullingMode::CullClockwise,
                    viewport: Some(viewport),
                    ..Default::default()
                };

                let cursor_pos = chessjam::grid_to_world(tile_cursor)
                    + vec3(0.0, 0.125, 0.0);
                let model_matrix = Mat4::translation(cursor_pos);
                let normal_matrix = Mat3::<f32>::identity();
                let mvp_matrix = view_projection_matrix * model_matrix;

                frame
                    .draw(
                        &cube_mesh.vertices,
                        &cube_mesh.indices,
                        &model_shader,
                        &uniform!{
                            transform: mvp_matrix.0,
                            normal_matrix: normal_matrix.0,
                            light_direction_matrix: light_direction_matrix.0,
                            light_color_matrix: light_color_matrix.0,
                            albedo: Vec4::from_slice(&config.colors.cursor).as_f32().0,
                        },
                        &cursor_draw_params,
                    )
                    .unwrap();
            }

            frame.finish().unwrap();
        }
    }
}
