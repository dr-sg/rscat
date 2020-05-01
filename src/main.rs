#[macro_use]
extern crate log;

use nalgebra;

mod rendering;

use std::io;
use std::io::BufRead;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

enum MouseMode {
    Cursor,
    CameraLook,
    CameraPan,
}


fn main() {
    
    let mut lines = Vec::<rendering::Line>::new();
    lines.push(rendering::defaults::get_random_walk(1.0,0.0,0.0,1000000));
    lines.push(rendering::defaults::get_random_walk(0.0,1.0,0.0,1000000));
    lines.push(rendering::defaults::get_random_walk(0.0,0.0,1.0,1000000));

    let mut vertices = rendering::defaults::get_sinc_vertices();
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

    event_loop.run(move |event, _, control_flow| {
        // If we have time-varying data, poll as fast as possible so we can update.
        //*control_flow = ControlFlow::Poll;

        // If we don't have any time varying data right now, start sleeping when we don't need to work.
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::DroppedFile(path),
                ..
            } => {
                lines.clear();
                let result = file_to_vertices(&path);
                if result.is_ok() {
                    let vertices = result.unwrap();
                    let line = rendering::Line{        
                        indicies: rendering::defaults::render_all_vertices(&vertices),
                        verticies: vertices,
                    }; 
                    lines.push(line)
                } else {
                    error!("Input contained invalid data: {}", path.as_path().display());
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                info!("Received WindowEvent::CloseRequested - Closing");
                *control_flow = ControlFlow::Exit
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { .. },
                ..
            } => {}
            Event::WindowEvent {
                event: WindowEvent::MouseWheel { delta, .. },
                ..
            } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        renderer
                            .camera
                            .move_focus(nalgebra::Vector2::<f32>::new(-x, 0.0));
                        renderer.camera.move_longitudinally(y);
                    }
                    _ => {} // TODO: Handle this arm
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
            } => match state {
                winit::event::ElementState::Pressed => match modifiers {
                    m if m.shift() => mouse_mode = MouseMode::CameraPan,
                    _ => mouse_mode = MouseMode::CameraLook,
                },
                winit::event::ElementState::Released => {
                    mouse_mode = MouseMode::Cursor;
                }
            },
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
                prev_mouse = position;
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                // Redraw the application.
                let frame = renderer
                    .swap_chain
                    .get_next_texture()
                    .expect("Timeout when acquiring next swap chain texture");
                let mut commands = renderer
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                renderer.render(&mut commands, &frame.view, &rendering::defaults::axes(), &rendering::defaults::render_all_vertices(&rendering::defaults::axes()), true);
                //renderer.render(&mut commands, &frame.view, &vertices, &indecies, false);
                for i in 0..lines.len() {
                    let v = &lines[i].verticies;
                    let i = &lines[i].indicies;
                    renderer.render(&mut commands, &frame.view, v, &i, false);
                }
                
                renderer.queue.submit(&[commands.finish()]);
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
