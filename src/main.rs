#![allow(unknown_lints)] // TODO(claire): Can go when clippy lints go.

extern crate chessjam;

#[macro_use]
extern crate static_assets;
#[macro_use]
extern crate glium;

extern crate adequate_math;

mod graphics;
mod input;

use adequate_math::*;
use glium::Display;
use glium::glutin::EventsLoop;

use input::*;


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

    let cube_mesh = graphics::create_cube_mesh(display, vec3(1.0, 1.0, 1.0));

    let mut frame_time = Instant::now();
    let mut keyboard = Keyboard::default();


    const TARGET_ASPECT: f32 = 16.0 / 9.0;
    let projection_matrix = matrix::perspective_projection(
        TARGET_ASPECT,
        consts::TAU32 * config.camera.fov as f32,
        0.1,
        100.0,
    );

    let view_matrix = {
        let position = vec3(0.0, config.camera.height, -config.camera.distance).as_f32();
        let focus = vec3(0.0, 0.0, 0.0);
        let direction = focus - position;
        let orientation = matrix::look_rotation(direction, vec3(0.0, 1.0, 0.0));
        let translation = Mat4::translation(-position);
        orientation.transpose() * translation
    };

    let view_projection_matrix = projection_matrix * view_matrix;

        let light_direction_matrix: Mat4<f32> = {
            let key = Vec4::from_slice(&config.light.key_dir).norm().as_f32();
            let fill = Vec4::from_slice(&config.light.fill_dir).norm().as_f32();
            let back = Vec4::from_slice(&config.light.back_dir).norm().as_f32();

            Mat4([key.0, fill.0, back.0, [0.0, 0.0, 0.0, 1.0]]).transpose()
        };

        let light_color_matrix: Mat4<f32> = Mat4([
            Vec4::from_slice(&config.light.key_color).as_f32().0,
            Vec4::from_slice(&config.light.fill_color).as_f32().0,
            Vec4::from_slice(&config.light.back_color).as_f32().0,
            Vec4::from_slice(&config.light.amb_color).as_f32().0,
        ]);

    loop {
        let (_dt, now) = chessjam::delta_time(frame_time);
        frame_time = now;

        // handle_events
        let mut closed = false;
        {
            use glium::glutin::{ElementState, Event, WindowEvent};

            let mut keyboard = keyboard.begin_frame_input();

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

        // render
        {
            use glium::{
                BackfaceCullingMode, Depth, DepthTest, DrawParameters, Rect,
                Surface,
            };
            use graphics::Mesh;

            struct RenderCommand<'a> {
                position: Vec3<f32>,
                mesh: &'a Mesh,
                color: Vec4<f32>,
            }

            let mut render_buffer = Vec::with_capacity(100);
            for y in 0..8 {
                for x in 0..8 {
                    let position = vec3(-3.5, -0.5, -3.5) + vec3(x, 0, y).as_f32();
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

            let mut frame = display.draw();
            frame.clear_color_srgb(0.0, 0.0, 0.0, 1.0);

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

            frame.clear(
                Some(&viewport),
                Some((0.3, 0.3, 0.3, 1.0)),
                true,
                Some(1.0),
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
                            light_color_matrix: light_color_matrix.0,
                            albedo: command.color.0,
                        },
                        &draw_params,
                    )
                    .unwrap();
            }

            frame.finish().unwrap();
        }
    }
}
