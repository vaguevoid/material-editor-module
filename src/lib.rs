mod asset_loading;

use crate::asset_loading::register_texture;

use eframe::egui;


use game_asset::ecs_module::GpuInterface;
use game_module_macro::{init, system, system_once, Component, ResourceWithoutSerialize};
use rand::prelude::ThreadRng;
use rand::Rng;
use std::ffi::CString;
use std::fs;
use std::ops::Range;
use std::path::Path;
use void_public::coordinate_systems::set_world_position;
use void_public::event::graphics::NewTexture;
use void_public::event::input::KeyCode;
use void_public::graphics::{TextureId, TextureRender};
use void_public::input::InputState;
use void_public::*;

const PLAYER_SPEED: f32 = 1_f32;
const MAX_AGENT_SPEED: f32 = 15.0;

const SMALL_TRAP_DISTANCE: f32 = 2.5_f32;
const SMALL_TRAP_MOVEMENT_MIN: f32 = 0.5_f32;
const SMALL_TRAP_MOVEMENT_MAX: f32 = 2.0_f32;
const AGENT_TIMER_RANGE: Range<f32> = 2.0..4.0;

const TWO_PI: f32 = 2.0 * std::f32::consts::PI;
const ROTATION_SPEED: f32 = 2.0;
const CAMERA_ZOOM_SPEED: f32 = 2f32;
const CAMERA_MOVE_SPEED: f32 = 200_f32;
const MAX_ZOOM: f32 = 100f32;

#[derive(ResourceWithoutSerialize)]
struct CustomResource {
    player_sprite_asset_ids: [TextureId; 4],
    trap_asset_id: TextureId,
    star_asset_id: TextureId,
    pub num_players: u32,
}

impl Default for CustomResource {
    fn default() -> Self {
        Self {
            player_sprite_asset_ids: [TextureId(0), TextureId(0), TextureId(0), TextureId(0)],
            trap_asset_id: TextureId(0),
            star_asset_id: TextureId(0),
            num_players: 4,
        }
    }
}

#[repr(C)]
#[derive(Component, Default, serde::Deserialize)]
struct PlayerInput {
    #[serde(default)]
    pub movement_input: Vec2,
    #[serde(default)]
    pub zoom_delta: f32,
    #[serde(default)]
    pub is_firing: bool,
    #[serde(default)]
    pub is_free_camera: bool,
    #[serde(default)]
    pub camera_movement_input: Vec2,
    #[serde(default)]
    pub active_player: u32,
}

#[repr(C)]
#[derive(Component, Default, serde::Deserialize)]
struct Player {
    #[serde(default)]
    look_dir: Vec2,
    #[serde(default)]
    is_dead: bool,
    #[serde(default)]
    is_started: bool,
    #[serde(default)]
    time_alive: f32,
}

#[repr(C)]
#[derive(Component, Default, serde::Deserialize)]
struct Shield {
    #[serde(default)]
    rotation_speed: f32,
}

#[repr(C)]
#[derive(Component, Debug, Default, serde::Deserialize)]
struct Velocity(Vec2);

#[repr(C)]
#[derive(Component, Debug, Default, serde::Deserialize)]
struct Timer {
    #[serde(default)]
    pub time_remaining: f32,
}
/*
#[derive(ResourceWithoutSerialize)]
struct MaterialEditorGui {
    eFrame::
}*/

#[system_once]
fn register_assets(
    gpu_interface: &mut GpuInterface,
    custom_resource: &mut CustomResource,
    new_texture_event_writer: EventWriter<NewTexture>,
    _aspect: &Aspect,
) {/*
    custom_resource.player_sprite_asset_ids[0] = register_texture(
        "textures/player_front.png",
        true,
        gpu_interface,
        &new_texture_event_writer,
    );*/
}

struct MyApp {}

impl Default for MyApp {
    fn default() -> Self {
        Self {}
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Hello, world!");
        });
    }
}


#[init]
fn init_gui() {
    std::thread::spawn(|| {
        eframe::run_native("My App", eframe::NativeOptions::default(), Box::new(|cc| Box::new(MyApp::default())));
    });
    

}

#[system_once]
fn spawn_scene(custom_resource: &mut CustomResource) {
    // spawn_input_capture();

    custom_resource.num_players = 4;
    match std::env::current_dir() {
        Ok(path) => println!("The current working directory is: {}", path.display()),
        Err(e) => eprintln!("Error getting current directory: {}", e),
    }

    let scene_str = fs::read_to_string(Path::new("../engine/target/debug/assets/scene.json"));
    assert!(scene_str.is_ok());

    let scene_str = scene_str.unwrap();
    let c_string = CString::new(scene_str).unwrap();
    let c_str = c_string.as_c_str();
    Engine::load_scene(c_str);

    let a = PlayerInput::default();
    Engine::spawn(bundle!(&a));
}

#[system]
fn capture_input(
    input_state: &InputState,
    custom_resource: &mut CustomResource,
    aspect: &Aspect,
    mut query_player_input: Query<&mut PlayerInput>,
) {
    let mut input_dir = Vec2::ZERO;
    let mut cam_input_dir = Vec2::ZERO;
    // println!("CAPTURING INPUT!");
    if key_is_down(input_state, KeyCode::KeyA) {
        input_dir.x -= 1.0;
    }
    if key_is_down(input_state, KeyCode::KeyD) {
        input_dir.x += 1.0;
    }
    if key_is_down(input_state, KeyCode::KeyW) {
        input_dir.y += 1.0;
    }
    if key_is_down(input_state, KeyCode::KeyS) {
        input_dir.y -= 1.0;
    }

    if key_is_down(input_state, KeyCode::ArrowLeft) {
        cam_input_dir.x -= 1.0;
    }
    if key_is_down(input_state, KeyCode::ArrowRight) {
        cam_input_dir.x += 1.0;
    }
    if key_is_down(input_state, KeyCode::ArrowUp) {
        cam_input_dir.y += 1.0;
    }
    if key_is_down(input_state, KeyCode::ArrowDown) {
        cam_input_dir.y -= 1.0;
    }

    //println!("{} ", query_player_input.len());

    if let Some(mut binding) = query_player_input.get_mut(0) {
        let player_input = binding.unpack();
        input_dir = input_dir.normalize_or_zero();
        player_input.movement_input = input_dir;

        if input_state.mouse.buttons.0[0].just_pressed() {
            let half_width = aspect.width / 2.0;
            let half_height = aspect.height / 2.0;
            match custom_resource.num_players {
                1 => {
                    player_input.active_player = 0;
                }
                2 => {
                    player_input.active_player = if input_state.mouse.cursor_position.x < half_width
                    {
                        0
                    } else {
                        1
                    };
                }
                3 => {
                    if input_state.mouse.cursor_position.x < half_width {
                        if input_state.mouse.cursor_position.y < half_height {
                            player_input.active_player = 2;
                        } else {
                            player_input.active_player = 0;
                        }
                    } else {
                        player_input.active_player = 1;
                    }
                }
                4 => {
                    if input_state.mouse.cursor_position.x < half_width {
                        if input_state.mouse.cursor_position.y < half_height {
                            player_input.active_player = 2;
                        } else {
                            player_input.active_player = 0;
                        }
                    } else {
                        if input_state.mouse.cursor_position.y < aspect.height / 2.0 {
                            player_input.active_player = 3;
                        } else {
                            player_input.active_player = 1;
                        }
                    }
                }
                _ => {}
            }
        }

        if key_just_pressed(input_state, KeyCode::Digit1) {
            player_input.active_player = 0;
        } else if key_just_pressed(input_state, KeyCode::Digit2) {
            player_input.active_player = 1;
        } else if key_just_pressed(input_state, KeyCode::Digit3) {
            player_input.active_player = 2;
        } else if key_just_pressed(input_state, KeyCode::Digit4) {
            player_input.active_player = 3;
        }

        if key_just_pressed(input_state, KeyCode::Space) {
            player_input.is_firing = true;
        }

        player_input.zoom_delta = input_state.mouse.scroll_delta.y;

        if key_just_pressed(input_state, KeyCode::Backquote) {
            player_input.is_free_camera = !player_input.is_free_camera;
        }

        // println!(" {} {}", input_dir, cam_input_dir);

        player_input.camera_movement_input = cam_input_dir;
    }
}

#[system]
fn process_input(
    // custom_resource: &CustomResource,
    mut query_player_input: Query<&mut PlayerInput>,
    mut query_player: Query<(&EntityId, &mut Transform, &LocalToWorld, &mut Player)>,
    mut query_cam: Query<(&mut Transform, &mut Camera)>,
    frame_constants: &FrameConstants,
) {
    let Some(mut binding) = query_player_input.get_mut(0) else {
        return;
    };

    //  println!("PROCESS INPUY");
    let player_input = binding.unpack();
    let movement_input = player_input.movement_input;
    let cam_input = player_input.camera_movement_input;

    if let Some(mut binding) = query_cam.get_mut(player_input.active_player as usize) {
        //  println!("  PROCESS INPUt");

        let (cam_transform, cam) = binding.unpack();
        cam.orthographic_size +=
            player_input.zoom_delta * frame_constants.delta_time * CAMERA_ZOOM_SPEED;
        cam.orthographic_size = cam.orthographic_size.clamp(1f32, MAX_ZOOM);

        {
            //if player_input.is_free_camera {
            let delta_cam_pos =
                (cam_input * frame_constants.delta_time * CAMERA_MOVE_SPEED).extend(0.0);
            cam_transform.position += delta_cam_pos;
        }
    }

    if let Some(mut binding) = query_player.get_mut(player_input.active_player as usize) {
        println!("  PROCESS -----");

        let (_player_entity, transform, _player_local_to_world, player_info) = binding.unpack();

        if movement_input != Vec2::ZERO {
            if !player_info.is_started {
                player_info.time_alive = 0f32;
            }
            player_info.is_started = true;
            player_info.look_dir = movement_input;
        }

        let delta_pos = movement_input * PLAYER_SPEED;
        let new_local_pos = transform.position.xy() + delta_pos;
        transform.position = new_local_pos.extend(0f32);
    }

    player_input.is_firing = false;
}

fn key_just_pressed(input_state: &InputState, key_just_pressed: KeyCode) -> bool {
    input_state.keys[key_just_pressed].just_pressed()
}

fn key_is_down(input_state: &InputState, key_code: KeyCode) -> bool {
    input_state.keys[key_code].pressed()
}

// This includes auto-generated C FFI code (saves you from writing it manually).
include!(concat!(env!("OUT_DIR"), "/ffi.rs"));
