use nalgebra_glm::Vec2;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, VirtualKeyCode},
};

use crate::{interface::interface::Interface, uniform::Uniform, tree::octree::Octree, Pref};

#[derive(PartialEq, Clone, Copy)]
pub enum Action {
    NONE,

    FORWARD,
    BACKWARD,
    LEFT,
    RIGHT,

    JUMP,
    SHIFT,

    FULLSCREEN,
    ESCAPE,

    RESET,
}

pub struct Input {
    pub binding_list: [Action; 256],
    pub key_down: [bool; 256],
}

impl Input {
    pub fn new() -> Input {
        let mut binding_list = [Action::NONE; 256];

        binding_list[VirtualKeyCode::W as usize] = Action::FORWARD;
        binding_list[VirtualKeyCode::S as usize] = Action::BACKWARD;
        binding_list[VirtualKeyCode::A as usize] = Action::LEFT;
        binding_list[VirtualKeyCode::D as usize] = Action::RIGHT;

        binding_list[VirtualKeyCode::Space as usize] = Action::JUMP;
        binding_list[VirtualKeyCode::LShift as usize] = Action::SHIFT;

        binding_list[VirtualKeyCode::F as usize] = Action::FULLSCREEN;
        binding_list[VirtualKeyCode::Escape as usize] = Action::ESCAPE;

        binding_list[VirtualKeyCode::R as usize] = Action::RESET;

        Input { binding_list, key_down: [false; 256] }
    }

    pub fn handle_key_input(
        &mut self,
        keycode: &VirtualKeyCode,
        state: &ElementState,
        uniform: &mut Uniform,
        pref: &Pref,
        octree: &Octree,
        interface: &Interface,
    ) {
        if state == &ElementState::Pressed {
            self.key_down[*keycode as usize] = true;
            /*
            match self.binding_list[*keycode as usize] {
                Action::FORWARD => uniform.velocity += nalgebra_glm::normalize(&uniform.look_dir) * pref.mov_speed,
                Action::BACKWARD => uniform.velocity -= nalgebra_glm::normalize(&uniform.look_dir) * pref.mov_speed,
                Action::LEFT => uniform.velocity -= normalize(&cross(&nalgebra_glm::normalize(&uniform.look_dir), &uniform.cam_up)) * pref.mov_speed,
                Action::RIGHT => uniform.velocity += normalize(&cross(&nalgebra_glm::normalize(&uniform.look_dir), &uniform.cam_up)) * pref.mov_speed,

                // Action::JUMP => uniform.apply_velocity(Vector3::new(0.0, MOVEMENT_INC, 0.0), octree),
                // Action::SHIFT => uniform.apply_velocity(Vector3::new(0.0, -MOVEMENT_INC, 0.0), octree),

                Action::FULLSCREEN => interface.window.set_fullscreen(Some(Fullscreen::Exclusive(
                    interface
                        .monitor
                        .video_modes()
                        .next()
                        .expect("ERR_NO_MONITOR_MODE")
                        .clone(),
                ))),
                Action::ESCAPE => interface.window.set_fullscreen(None),
                Action::RESET => interface
                    .window
                    .set_cursor_position(PhysicalPosition::new(
                        uniform.res.x / 2.0,
                        uniform.res.x / 2.0,
                    ))
                    .unwrap(),

                _ => (),
            }
            */
        } else if state == &ElementState::Released {
            self.key_down[*keycode as usize] = false;
        }
    }

    pub fn handle_mouse_input(&self, position: PhysicalPosition<f64>, uniform: &mut Uniform) {
        let mouse_pos = Vec2::new(position.x as f32, position.y as f32);
        let mouse_delta = mouse_pos - uniform.res / 2.0;

        uniform.move_mouse(mouse_delta);
    }
}
