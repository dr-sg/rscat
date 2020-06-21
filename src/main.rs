#![deny(warnings)]
#[macro_use]
extern crate log;

use nalgebra;

mod rendering;

use std::io::BufRead;
use std::time::Instant;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use imgui::*;
use imgui_winit_support;

enum MouseMode {
    Cursor,
    CameraLook,
    CameraPan,
}

struct UiState {
    camera_target: [f32; 3],
    camera_range: f32,
    camera_azimuth: f32,
    camera_elevation: f32,
    gui_open: bool,
}

impl UiState {
    pub fn default() -> Self {
        return UiState {
            camera_target: [0.0, 0.0, 0.0],
            camera_range: 10.0,
            camera_azimuth: 0.0,
            camera_elevation: 0.0,
            gui_open: false,
        };
    }
}

fn main() {
    let mut lines = Vec::<rendering::Line>::new();
    lines.push(rendering::defaults::get_random_walk(1.0, 0.0, 0.0, 1000000));
    lines.push(rendering::defaults::get_random_walk(0.0, 1.0, 0.0, 1000000));
    lines.push(rendering::defaults::get_random_walk(0.0, 0.0, 1.0, 1000000));

    let vertices = rendering::defaults::get_sinc_vertices();
    let line = rendering::Line{        
        indicies: rendering::defaults::render_all_vertices(&vertices),
        verticies: vertices,
    };

    lines.push(line);

    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Rapid Scene Composition & Analysis Tool")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
        .build(&event_loop)
        .unwrap();

    let size = window.inner_size();

    let surface = wgpu::Surface::create(&window);

    let mut renderer = rendering::Renderer::new(surface, size);

    let mut prev_mouse = winit::dpi::PhysicalPosition::new(0.0, 0.0);
    let mut mouse_mode = MouseMode::Cursor;
    let mut modifiers = winit::event::ModifiersState::empty();

    let mut ui_state = UiState::default();
    let mut prev_frame_time = Instant::now();
    let mut last_cursor = None;


    let mut hidpi_factor = 1.0;
    let mut imgui_context = imgui::Context::create();
    let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui_context);
    platform.attach_window(
        imgui_context.io_mut(),
        &window,
        imgui_winit_support::HiDpiMode::Default,
    );
    imgui_context.set_ini_filename(None);

    let font_size = (13.0 * hidpi_factor) as f32;
    imgui_context.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    imgui_context.fonts().add_font(&[FontSource::DefaultFontData {
        config: Some(imgui::FontConfig {
            oversample_h: 1,
            pixel_snap_h: true,
            size_pixels: font_size,
            ..Default::default()
        }),
    }]);

    let mut imgui_renderer = imgui_wgpu::Renderer::new(
        &mut imgui_context,
        &renderer.device,
        &mut renderer.queue,
        renderer.sc_desc.format,
        None,
    );

    event_loop.run(move |event, _, control_flow| {
        // If we have time-varying data, poll as fast as possible so we can update.
        //*control_flow = ControlFlow::Poll;

        // If we don't have any time varying data right now, start sleeping when we don't need to work.
        *control_flow = ControlFlow::Wait;

        if ui_state.gui_open {
            // Have imgui_context handle the event first
            platform.handle_event(imgui_context.io_mut(), &window, &event);
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::DroppedFile(path),
                ..
            } => {
                lines.clear();
                let result = file_to_vertices(&path);
                if result.is_ok() {
                    let vertices = result.unwrap();
                    let line = rendering::Line {
                        indicies: rendering::defaults::render_all_vertices(&vertices),
                        verticies: vertices,
                    };
                    lines.push(line)
                } else {
                    error!("Input contained invalid data: {}", path.as_path().display());
                }
            }
            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { scale_factor, ..},
                ..
            } => {
                hidpi_factor = scale_factor;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                info!("Received WindowEvent::CloseRequested - Closing");
                *control_flow = ControlFlow::Exit
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                if !imgui_context.io().want_capture_keyboard {
                    if input.state == winit::event::ElementState::Released {
                        match input.virtual_keycode {
                            Some(winit::event::VirtualKeyCode::Grave) => ui_state.gui_open = !ui_state.gui_open,
                            _ => {}
                        }
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::MouseWheel { delta, .. },
                ..
            } => {
                if !imgui_context.io().want_capture_mouse {
                    match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => {
                            renderer
                                .camera
                                .move_focus(nalgebra::Vector2::<f32>::new(-x, 0.0));
                            renderer.camera.move_longitudinally(y);
                            ui_state.camera_range = renderer.camera.range;
                        }
                        _ => {} // TODO: Handle this arm
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                renderer.resize(size);
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        device_id: _,
                        state,
                        button: _,
                        ..
                    },
                ..
            } => {
                if !imgui_context.io().want_capture_mouse {
                    match state {
                        winit::event::ElementState::Pressed => match modifiers {
                            m if m.shift() => mouse_mode = MouseMode::CameraPan,
                            _ => mouse_mode = MouseMode::CameraLook,
                        },
                        winit::event::ElementState::Released => {
                            mouse_mode = MouseMode::Cursor;
                        }
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::ModifiersChanged(modifiers_state),
                ..
            } => {
                modifiers = modifiers_state;
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                let mouse_delta = nalgebra::Vector2::<f32>::new(
                    (position.x - prev_mouse.x) as f32,
                    (position.y - prev_mouse.y) as f32,
                );
                match &mouse_mode {
                    MouseMode::Cursor => {}
                    MouseMode::CameraLook => renderer.camera.move_on_orbit(mouse_delta),
                    MouseMode::CameraPan => renderer.camera.move_focus(mouse_delta),
                }
                ui_state.camera_target[0] = renderer.camera.target[0];
                ui_state.camera_target[1] = renderer.camera.target[1];
                ui_state.camera_target[2] = renderer.camera.target[2];
                ui_state.camera_azimuth = renderer.camera.azimuth;
                ui_state.camera_elevation = renderer.camera.elevation;
                prev_mouse = position;
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                // Redraw the application.
                let frame_time_delta = prev_frame_time.elapsed();
                prev_frame_time = imgui_context.io_mut().update_delta_time(prev_frame_time);

                let frame = renderer
                    .swap_chain
                    .get_next_texture()
                    .expect("Timeout when acquiring next swap chain texture");
                let mut commands = renderer
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                renderer.render(
                    &mut commands,
                    &frame.view,
                    &rendering::defaults::axes(),
                    &rendering::defaults::render_all_vertices(&rendering::defaults::axes()),
                    true,
                );
                for i in 0..lines.len() {
                    let v = &lines[i].verticies;
                    let i = &lines[i].indicies;
                    renderer.render(&mut commands, &frame.view, v, &i, false);
                }
                
                let mut imgui_commands: wgpu::CommandEncoder = renderer
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                if ui_state.gui_open {

                platform
                    .prepare_frame(imgui_context.io_mut(), &window)
                    .expect("Failed to prepare frame");
                let ui = imgui_context.frame();

                {
                    let window = imgui::Window::new(im_str!("Display"));
                    window
                        .size([300.0, 100.0], Condition::FirstUseEver)
                        .build(&ui, || {
                            let mouse_pos = ui.io().mouse_pos;
                            ui.text(im_str!(
                                "Mouse Position: ({:.1},{:.1})",
                                mouse_pos[0],
                                mouse_pos[1]
                            ));
                            ui.text(im_str!("Frametime: {:?}", frame_time_delta));
                        });

                    let window = imgui::Window::new(im_str!("Camera"));
                    window
                        .size([400.0, 200.0], Condition::FirstUseEver)
                        .position([400.0, 200.0], Condition::FirstUseEver)
                        .build(&ui, || {
                            //ui.list_box()
                            if ui
                                .input_float3(im_str!("Target"), &mut ui_state.camera_target)
                                .build()
                            {
                                renderer.camera.set_target(nalgebra::Point3::<f32>::new(
                                    ui_state.camera_target[0],
                                    ui_state.camera_target[1],
                                    ui_state.camera_target[2],
                                ));
                            }
                            if ui
                                .input_float(im_str!("Range"), &mut ui_state.camera_range)
                                .build()
                            {
                                println!("Range changed");
                                renderer.camera.set_range(ui_state.camera_range);
                            }
                            if ui
                                .input_float(im_str!("Azimuth"), &mut ui_state.camera_azimuth)
                                .build()
                            {
                                println!("Az changed");
                                renderer.camera.set_azimuth(ui_state.camera_azimuth);
                            }
                            if ui
                                .input_float(im_str!("Elevation"), &mut ui_state.camera_elevation)
                                .build()
                            {
                                renderer.camera.set_elevation(ui_state.camera_elevation);
                            }
                        });
                }

                if last_cursor != Some(ui.mouse_cursor()) {
                    last_cursor = Some(ui.mouse_cursor());
                    platform.prepare_render(&ui, &window);
                }
                imgui_renderer
                    .render(
                        ui.render(),
                        &mut renderer.device,
                        &mut imgui_commands,
                        &frame.view,
                    )
                    .expect("imgui_context rendering failed");
                }

                renderer
                    .queue
                    .submit(&[commands.finish(), imgui_commands.finish()]);
            }
            _ => {}
        }
    });
}

fn file_to_vertices(
    path: &std::path::PathBuf,
) -> Result<Vec<rendering::Vertex>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut vertices = Vec::<rendering::Vertex>::new();
    for line in reader.lines() {
        let line = line?;
        let split: Vec<&str> = line.split(',').collect();
        if split.len() != 7 {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Input needs 7 cols: X, Y, Z, R, G, B, Size",
            )));
        } else {
            vertices.push(rendering::Vertex {
                position: [
                    split[0].parse()?,
                    split[1].parse()?,
                    split[2].parse()?,
                    1.0_f32,
                ],
                color: [
                    split[3].parse()?,
                    split[4].parse()?,
                    split[5].parse()?,
                    1.0_f32,
                ],
                size: split[6].parse()?,
            });
        }
    }
    return Ok(vertices);
}
