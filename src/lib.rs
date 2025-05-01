use game_module_macro::{Component, system, system_once};

use once_cell::sync::Lazy;

use memmap2::MmapMut;
use std::{
    ffi::CString,
    fs::{self, OpenOptions},
    path::Path,
    process::Command,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use void_public::{event::input::KeyCode, input::InputState, *};

const PLAYER_SPEED: f32 = 1_f32;

const CAMERA_ZOOM_SPEED: f32 = 2f32;
const CAMERA_MOVE_SPEED: f32 = 200_f32;
const MAX_ZOOM: f32 = 100f32;

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

static SHARED_MEM_FILE: Lazy<Mutex<MmapMut>> = Lazy::new(|| {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("shared_memory.bin")
        .expect("Failed to open file");

    file.set_len(4096).expect("Failed to set file size");

    Mutex::new(unsafe { MmapMut::map_mut(&file).expect("Failed to mmap") })
});

#[system_once]
fn init_shared_mem() {
    let material_editor_gui = "./target/debug/material_editor_gui.exe";
    let _ = Command::new(material_editor_gui)
        .spawn()
        .expect("Failed to start Project B");
}

#[system_once]
fn spawn_scene() {
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
fn capture_input(input_state: &InputState, mut query_player_input: Query<&mut PlayerInput>) {
    unsafe {
        if let Ok(mut shared_mem) = SHARED_MEM_FILE.try_lock() {
            let read_barrier = { &*(shared_mem.as_ptr() as *mut AtomicBool) };

            if !read_barrier.load(Ordering::Acquire) {
                let incoming_message =
                    std::str::from_utf8(&shared_mem[1..]).expect("Invalid UTF-8");
                println!("Engine - Incoming message = {incoming_message}");

                let msg = b"Engine Frame Count is {FRAME_COUNTER}";
                shared_mem[1..msg.len() + 1].copy_from_slice(msg);
                read_barrier.store(true, Ordering::Release);
            }

            shared_mem.flush().expect("Failed to flush");
        }
    }

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
            player_input.active_player = 0;
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

/*
pub fn register_texture(
    texture_path: &str,
    load_into_atlas: bool,
    gpu_interface: &mut GpuInterface,
    new_texture_event_writer: &EventWriter<NewTexture>,
) -> TextureId {
    let id = gpu_interface
        .texture_asset_manager
        .register_next_texture_id();
    let pending_texture = PendingTexture::new(id, &texture_path.into(), load_into_atlas);
    gpu_interface
        .texture_asset_manager
        .load_texture_by_pending_texture(&pending_texture, new_texture_event_writer)
        .unwrap();
    id
}

*/
// This includes auto-generated C FFI code (saves you from writing it manually).
include!(concat!(env!("OUT_DIR"), "/ffi.rs"));
