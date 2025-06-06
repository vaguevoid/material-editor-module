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

use game_asset::{
    ecs_module::GpuInterface,
    resource_managers::{
        material_manager::{
            DEFAULT_SHADER_ID, material_parameters_extension::MaterialParametersExt,
            uniforms::UniformValue,
        },
        texture_asset_manager::PendingTexture,
    },
};
use game_module_macro::{Component, ResourceWithoutSerialize, system, system_once};
use gpu_web::{GpuResource, gpu_managers::texture_manager::RenderTargetType};
use memmap2::MmapMut;
use once_cell::sync::Lazy;

use void_public::{
    event::{graphics::NewTexture, input::KeyCode},
    graphics::{DefaultMaterials, MaterialId, MaterialParameters, TextureId, TextureRender},
    input::InputState,
    *,
};

const CAMERA_ZOOM_SPEED: f32 = 2.;
const CAMERA_MOVE_SPEED: f32 = 200.;
const MAX_ZOOM: f32 = 100.;

static SHARED_MEM_FILE: Lazy<Mutex<MmapMut>> = Lazy::new(|| {
    println!("Opening shared file...");

    match std::env::current_dir() {
        Ok(path) => println!("  The current working directory is: {}", path.display()),
        Err(e) => eprintln!("   Error getting current directory: {}", e),
    }

    let _ = std::fs::create_dir("./temp/");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open("./temp/shared_memory.bin")
        .expect("Failed to open file");
    file.set_len(131072).expect("Failed to set file size");

    Mutex::new(unsafe { MmapMut::map_mut(&file).expect("Failed to mmap") })
});

#[repr(C)]
#[derive(Component, Default, serde::Deserialize)]
struct UserInput {
    #[serde(skip_deserializing)]
    pub zoom_delta: f32,
    #[serde(skip_deserializing)]
    pub camera_movement_input: Vec2,
}

#[derive(ResourceWithoutSerialize)]
struct MaterialEditor {
    material_id: MaterialId,
}

impl Default for MaterialEditor {
    fn default() -> Self {
        MaterialEditor {
            material_id: MaterialId(0),
        }
    }
}

#[system_once]
fn initialize_module() {
    println!("Initializing Material Editor module.");

    #[cfg(target_os = "macos")]
    if !Path::new("./target/debug/").exists() {
        let exe_path = std::env::current_exe().expect("Failed to get executable path");
        let app_dir = exe_path.parent().expect("Failed to get parent directory");
        std::env::set_current_dir(app_dir).expect("Failed to set current directory");
    }

    // Init shared mem
    if let Ok(mut shared_mem) = SHARED_MEM_FILE.try_lock() {
        shared_mem[0..].fill(b'\0');
    }

    // Open the gui
    let material_editor_gui = {
        let path = if Path::new("./target/debug/").exists() {
            "./target/debug/material_editor_gui.exe"
        } else {
            "./material_editor_gui.exe"
        };

        #[cfg(target_os = "macos")]
        let path = path.strip_suffix(".exe").unwrap_or(path);

        path
    };

    println!("Spawning process {material_editor_gui}");
    let _ = Command::new(material_editor_gui)
        .spawn()
        .expect("Failed to start Material Editor Gui");

    // Load scene
    let scene_path = "../engine/target/debug/assets/scene.json";
    let scene_str = if Path::new(scene_path).exists() {
        fs::read_to_string("../engine/target/debug/assets/scene.json")
    } else {
        fs::read_to_string("./assets/scene.json")
    };

    assert!(scene_str.is_ok());

    let scene_str = scene_str.unwrap();
    let c_string = CString::new(scene_str).unwrap();
    let c_str = c_string.as_c_str();
    Engine::load_scene(c_str);

    // User input
    let user_input = UserInput {
        zoom_delta: 0.,
        camera_movement_input: Vec2::new(0., 0.),
    };
    Engine::spawn(bundle!(&user_input));
}

#[system]
fn update_loop(
    gpu_interface: &mut GpuInterface,
    material_editor: &mut MaterialEditor,
    gpu_resource: &mut GpuResource,
    mut texture_query: Query<(&TextureRender, &mut MaterialParameters)>,
    new_texture_event_writer: EventWriter<NewTexture>,
) {
    let mut new_material_id: Option<MaterialId> = None;
    let mut new_tex_id: Option<TextureId> = None;
    let mut param_update: Option<(String, Vec4)> = None;

    // Acquire lock if available
    let shared_mem = SHARED_MEM_FILE.try_lock();
    if shared_mem.is_err() {
        return;
    }
    let mut shared_mem = shared_mem.unwrap();

    let read_barrier = unsafe { &*(shared_mem.as_ptr() as *mut AtomicBool) };
    if !read_barrier.load(Ordering::Acquire) {
        let outgoing_command = String::new();
        let incoming_message = std::str::from_utf8(&shared_mem[1..]).expect("Invalid UTF-8");

        if incoming_message.as_bytes()[0] != b'\0' {
            let incoming_command: Vec<&str> = incoming_message.split("##DELIM##").collect();
            if !incoming_command.is_empty() {
                match incoming_command[0] {
                    // Update Uniform variables
                    "update_uniform" => {
                        let color: [f32; 4] = {
                            let color_vec: Vec<f32> = incoming_command[2]
                                .split(',')
                                .map(|s| s.trim().parse::<f32>().unwrap_or(1.))
                                .collect();

                            color_vec.try_into().unwrap_or([1., 1., 1., 1.])
                        };
                        param_update = Some((incoming_command[1].to_string(), color.into()));
                    }

                    // Load texture
                    "load_texture" => {
                        let texture_path = incoming_command[1];
                        let id = if let Some(tex) = gpu_interface
                            .texture_asset_manager
                            .get_texture_by_path(&texture_path.into())
                        {
                            tex.id()
                        } else {
                            let id = gpu_interface
                                .texture_asset_manager
                                .register_next_texture_id();
                            let pending_texture =
                                PendingTexture::new(id, &texture_path.into(), false);
                            let _ = gpu_interface
                                .texture_asset_manager
                                .load_texture_by_pending_texture(
                                    &pending_texture,
                                    &new_texture_event_writer,
                                );
                            id
                        };

                        new_tex_id = Some(id);
                    }

                    // Compile material
                    "compile" => {
                        if let Some(_mat) = gpu_interface
                            .material_manager
                            .get_material(material_editor.material_id)
                        {
                            let compile_params: Vec<&str> =
                                incoming_message.split("##DELIM##").collect();

                            let toml_shader = format!(
                                "get_world_offset = \"\"\"\n{}\n\"\"\"\nget_fragment_color = \"\"\"\n{}\n\"\"\"\n[uniform_types]\n{}\n[texture_descs]\n{}",
                                compile_params[3],
                                compile_params[4],
                                compile_params[1],
                                compile_params[2],
                            );

                            // dbg!("---> {}", &toml_shader);
                            let mat_id = gpu_interface
                                .material_manager
                                .register_material_from_string(
                                    DEFAULT_SHADER_ID,
                                    "test_mat",
                                    &toml_shader,
                                );

                            if let Ok(material_id) = mat_id {
                                new_material_id = Some(material_id);
                                let resolve_target = gpu_resource
                                    .texture_manager
                                    .get_render_target(RenderTargetType::ColorResolve);

                                println!("Registering new material pipeline {material_id}");
                                gpu_resource.pipeline_manager.register_pipeline(
                                    material_id,
                                    resolve_target.texture.format(),
                                    4,
                                    &gpu_resource.device,
                                    &gpu_interface.material_manager,
                                    wgpu::BlendState::ALPHA_BLENDING,
                                );
                            } else {
                                println!("Unable to register new material pipeline {:?}", mat_id);
                                new_material_id =
                                    Some(DefaultMaterials::MissingOrBroken.material_id());
                            }
                        }
                    }
                    _ => {}
                };
            }

            // Clear shared_mem buffer
            shared_mem[1..].fill(b'\0');
        }

        // Write outgoing commands
        if !outgoing_command.is_empty() {
            shared_mem[1..outgoing_command.len() + 1].copy_from_slice(outgoing_command.as_bytes());
        }

        read_barrier.store(true, Ordering::Release);
    }

    shared_mem.flush().expect("Failed to flush");

    // Update sprites with any new data
    texture_query.for_each(|(_, parameters)| {
        if let Some(material_id) = new_material_id {
            let default_material = gpu_interface
                .material_manager
                .get_material(material_id)
                .unwrap();

            parameters.data = default_material.generate_default_material_parameters().data;
            parameters.material_id = new_material_id.unwrap();
        }

        if let Some(param) = &param_update {
            let uniform = UniformValue::Vec4(param.1.into());
            if let Ok(new_param) = parameters.update_uniforms(
                &gpu_interface.material_manager,
                &[(param.0.clone(), &uniform)],
            ) {
                parameters.data = new_param.data;
            }
        }
        if new_tex_id.is_some() {
            println!("Setting new tex id {}", new_tex_id.unwrap());
            parameters.textures[0] = new_tex_id.unwrap();
        }
    });
}

#[system]
fn capture_input(input_state: &InputState, mut query_player_input: Query<&mut UserInput>) {
    let mut input_dir = Vec2::ZERO;
    let mut cam_input_dir = Vec2::ZERO;

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

    if let Some(mut binding) = query_player_input.get_mut(0) {
        let player_input = binding.unpack();
        //input_dir = input_dir.normalize_or_zero();

        player_input.zoom_delta = input_state.mouse.scroll_delta.y;
        player_input.camera_movement_input = cam_input_dir;
    }
}

#[system]
fn process_input(
    mut query_player_input: Query<&mut UserInput>,
    mut query_cam: Query<(&mut Transform, &mut Camera)>,
    frame_constants: &FrameConstants,
) {
    let Some(mut binding) = query_player_input.get_mut(0) else {
        return;
    };

    let player_input = binding.unpack();
    let cam_input = player_input.camera_movement_input;

    if let Some(mut binding) = query_cam.get_mut(0) {
        let (cam_transform, cam) = binding.unpack();
        cam.orthographic_size +=
            player_input.zoom_delta * frame_constants.delta_time * CAMERA_ZOOM_SPEED;
        cam.orthographic_size = cam.orthographic_size.clamp(1f32, MAX_ZOOM);

        let delta_cam_pos =
            (cam_input * frame_constants.delta_time * CAMERA_MOVE_SPEED).extend(0.0);
        cam_transform
            .position
            .set(cam_transform.position.get() + delta_cam_pos);
    }
}

fn key_is_down(input_state: &InputState, key_code: KeyCode) -> bool {
    input_state.keys[key_code].pressed()
}

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

// This includes auto-generated C FFI code (saves you from writing it manually).
include!(concat!(env!("OUT_DIR"), "/ffi.rs"));
