use std::{
    borrow::BorrowMut,
    io::Write,
    thread,
    time::{Duration, Instant},
};

use ash::vk;
use env_logger::fmt::{Color, Formatter};
use input::Input;
use interface::Interface;
use log::Record;
use pipe::Pipe;
use tree::octree::Octree;
use uniform::Uniform;
use winit::{
    dpi::PhysicalPosition,
    event::{Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
};

mod input;
mod interface;
mod tree;
mod pipe;
mod service;
mod uniform;

const DEFAULT_STORAGE_BUFFER_SIZE: u64 = 1342177280;
const DEFAULT_UNIFORM_BUFFER_SIZE: u64 = 16384;

pub struct RenderState {
    pub out_of_date: bool,
    pub idle: bool,

    pub frame_time: Duration,
}

// Complete Render Pipeline
pub struct Render {
    state: RenderState,
    event_loop: EventLoop<()>,

    pref: Pref,
    uniform: Uniform,
    octree: Octree,

    input: Input,

    interface: Interface,
    graphic_pipe: Pipe,
}

// General Setting
pub struct Pref {
    pub pref_present_mode: vk::PresentModeKHR,
    pub img_filter: vk::Filter,
    pub img_scale: f32,

    pub name: String,
    pub engine_name: String,

    pub start_window_size: vk::Extent2D,

    pub use_render_res: bool,
    pub render_res: vk::Extent2D,
}

fn main() {
    let log_format = |buf: &mut Formatter, record: &Record| {
        let mut buf_style = buf.style();

        buf_style.set_color(Color::Yellow).set_bold(true);

        let time = chrono::Local::now().format("%H:%M:%S");

        writeln!(
            buf,
            "[ {} {} ] {}",
            time,
            buf_style.value(record.level()),
            record.args(),
        )
    };

    env_logger::builder().format(log_format).init();

    log::info!("Starting Application ...");
    thread::spawn(|| loop {});

    let mut render = Render::get_render();
    render.execute(Instant::now());
}

impl Render {
    pub fn get_render() -> Render {
        let event_loop = EventLoop::new();

        let pref = Pref {
            pref_present_mode: vk::PresentModeKHR::IMMEDIATE,
            img_filter: vk::Filter::LINEAR,
            img_scale: 1.0,

            name: env!("CARGO_PKG_NAME").to_string(),
            engine_name: "Engine".to_string(),

            start_window_size: vk::Extent2D {
                width: 800,
                height: 600,
            },

            use_render_res: true,
            render_res: vk::Extent2D {
                width: 1920,
                height: 1080,
            },
        };

        let state = RenderState {
            out_of_date: false,
            idle: false,
            frame_time: Duration::ZERO,
        };

        let mut octree = Octree::default();

        let input = Input::new();
        let mut uniform = Uniform::new(octree.root_span);
        
        octree.test_scene();

        let interface = Interface::init(&event_loop, &pref);
        let graphic_pipe = Pipe::init(&interface, &pref, &mut uniform, &octree);

        Render {
            state,
            event_loop,
            pref,
            uniform,
            octree,
            input,
            interface,
            graphic_pipe,
        }
    }

    pub fn execute(&mut self, app_start: Instant) {
        self.event_loop
            .borrow_mut()
            .run_return(|event, _, control_flow| {
                *control_flow = ControlFlow::Poll;
                match event {
                    Event::WindowEvent {
                        event:
                            WindowEvent::KeyboardInput {
                                input:
                                    KeyboardInput {
                                        virtual_keycode: Some(keycode),
                                        state,
                                        ..
                                    },
                                ..
                            },
                        ..
                    } =>
                    // Handle KeyboardInput
                    {
                        self.input.handle_key_input(
                            &keycode,
                            &state,
                            &self.interface,
                            &mut self.uniform,
                        )
                    }

                    Event::WindowEvent {
                        event: WindowEvent::CursorMoved { position, .. },
                        ..
                    } => {
                        self.input.handle_mouse_input(position, &mut self.uniform);
                        self.interface.window.set_cursor_visible(false);
                        self.interface
                            .window
                            .set_cursor_position(PhysicalPosition::new(
                                self.uniform.resolution.x / 2.0,
                                self.uniform.resolution.y / 2.0,
                            ))
                            .unwrap();
                    }

                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    Event::MainEventsCleared =>
                    // Adjust Surface and Draw
                    {
                        if self.state.out_of_date {
                            let dim = self.interface.window.inner_size();
                            if dim.width > 0 && dim.height > 0 {
                                // Not Minimized
                                self.graphic_pipe.recreate_swapchain(
                                    &mut self.interface,
                                    &mut self.uniform,
                                    &self.pref,
                                );

                                self.state.idle = false;
                                self.state.out_of_date = false;
                            } else {
                                // Minimized
                                self.state.idle = true;
                            }
                        } else {
                            // Update Octree
                            // self.octree = Octree::default();
                            // self.octree.test_scene();
                            // self.graphic_pipe.update_buffer(&self.interface, self.graphic_pipe.octree_buffer_memory, &self.octree.data.clone(), );

                            // Update Uniform
                            self.uniform.update_uniform(app_start.elapsed());
                            self.graphic_pipe
                                .uniform_buffer
                                .update(&self.interface, &[self.uniform]);

                            // Draw and capture FrameTime
                            let start = Instant::now();
                            self.state.out_of_date = self
                                .graphic_pipe
                                .draw(&self.interface, &self.pref, &self.uniform)
                                .expect("RENDER_FAILED");
                            self.state.frame_time = start.elapsed();
                        }
                    }

                    Event::LoopDestroyed => self.interface.wait_for_gpu().expect("DEVICE_LOST"),
                    _ => (),
                }
            });
    }
}
