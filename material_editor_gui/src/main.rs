// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![warn(static_mut_refs)]
#![allow(rustdoc::missing_crate_level_docs)]
use core::f32;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use eframe::egui::{self, CentralPanel, ComboBox, ScrollArea, TextEdit};
use memmap2::MmapMut;
use once_cell::sync::Lazy;
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use serde_json;

static MATERIAL_EDITOR_VERSION: u32 = 0;
static USER_SETTINGS_PATH: &str = "./temp/user_settings.json";
static MAX_TEXTURES: usize = 16;

static SHARED_MEM_FILE: Lazy<Mutex<MmapMut>> = Lazy::new(|| {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("./temp/shared_memory.bin")
        .expect("Failed to open file");

    Mutex::new(unsafe { MmapMut::map_mut(&file).expect("Failed to mmap") })
});

static mut GLOBAL_CONFIG: Option<UserSettings> = None;
#[allow(static_mut_refs)]
fn get_config() -> &'static mut UserSettings {
    unsafe { GLOBAL_CONFIG.as_mut().unwrap() }
}

// User Settings
#[derive(Serialize, Deserialize, Debug)]
struct UserSettings {
    version: u32,
    shader_directory: PathBuf,
    texture_directories: [PathBuf; MAX_TEXTURES],
}

struct MaterialEditor {
    shader_path: PathBuf,

    textures: [String; MAX_TEXTURES],

    textures_text: String,
    uniforms_text: String,
    world_offset_text: String,
    frag_color_text: String,
}

impl MaterialEditor {
    fn load_material(&mut self, file_path: &PathBuf) {
        self.shader_path = file_path.clone();
        get_config().shader_directory = self
            .shader_path
            .parent()
            .unwrap_or(PathBuf::from("./").as_path())
            .to_path_buf();

        if let Ok(material_toml) = fs::read_to_string(&self.shader_path) {
            let key = "[uniform_types]";
            if let Some(snippet_key_idx) = material_toml.find(key) {
                let snippet_start = snippet_key_idx + key.len();
                let snippet = &material_toml[snippet_start..];
                let snippet_end = [
                    "[texture_descs]",
                    "[get_world_offset]",
                    "[get_fragment_color]",
                ]
                .iter()
                .filter_map(|key| snippet.find(key))
                .min()
                .unwrap_or(snippet.len());

                self.uniforms_text = snippet[..snippet_end].trim_start().trim_end().to_string();
            }

            let key = "[texture_descs]";
            if let Some(snippet_key_idx) = material_toml.find(key) {
                let snippet_start = snippet_key_idx + key.len();
                let snippet = &material_toml[snippet_start..];
                let snippet_end = snippet.find('[').unwrap_or(snippet.len());
                self.textures_text = snippet[..snippet_end].trim_start().trim_end().to_string();
            }

            let key = "get_world_offset";
            if let Some(snippet_key_idx) = material_toml.find(key) {
                let snippet = &material_toml[snippet_key_idx..];

                let start = snippet.find("\"\"\"").unwrap();
                let end = snippet[start + 3..].find("\"\"\"").unwrap();
                self.world_offset_text = snippet[start + 3..start + 3 + end]
                    .trim_start()
                    .trim_end()
                    .to_string();
            }

            let key = "get_fragment_color";
            if let Some(snippet_key_idx) = material_toml.find(key) {
                let snippet = &material_toml[snippet_key_idx..];

                let start = snippet.find("\"\"\"").unwrap();
                let end = snippet[start + 3..].find("\"\"\"").unwrap();
                self.frag_color_text = snippet[start + 3..start + 3 + end]
                    .trim_start()
                    .trim_end()
                    .to_string();
            }
        }
    }

    fn save_material(&self, file_path: &PathBuf) {
        if let Ok(mut file) = File::create(file_path) {
            let toml_mat = format!(
                "get_world_offset = \"\"\"\n{}\n\"\"\"\n\nget_fragment_color = \"\"\"\n{}\"\"\"\n\n[uniform_types]\n{}\n\n[texture_descs]\n{}\n",
                self.world_offset_text
                    .trim_start()
                    .trim_end()
                    .replace("\r", "\n"),
                self.frag_color_text
                    .trim_start()
                    .trim_end()
                    .replace("\r", "\n"),
                self.uniforms_text
                    .trim_start()
                    .trim_end()
                    .replace("\r", "\n"),
                self.textures_text
                    .trim_start()
                    .trim_end()
                    .replace("\r", "\n"),
            );

            if let Err(result) = file.write_all(toml_mat.as_bytes()) {
                println!(
                    "Failed to write material {} with error {}",
                    file_path.to_string_lossy(),
                    result.to_string()
                );
            } else {
                println!("Saved material {}", file_path.to_string_lossy());
            }
        }
    }
}

impl Default for MaterialEditor {
    fn default() -> Self {
        let shader_path: PathBuf = env::current_dir().unwrap();

        Self {
            shader_path,
            textures: std::array::from_fn(|_| String::new()),
            uniforms_text: "".to_string(),
            textures_text: "".to_string(),
            world_offset_text: "".to_string(),
            frag_color_text: "".to_string(),
        }
    }
}

impl eframe::App for MaterialEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut cmd_string = String::new();
        let mut save_config = false;

        CentralPanel::default().show(ctx, |ui| {
            let available_rect = ctx.available_rect();
            let usable_width = ui.max_rect().width() - ui.spacing().item_spacing.x;
            let usable_height = ui.max_rect().height() - ui.spacing().item_spacing.y;

            let text_height = 12_f32;

            ui.set_min_size(available_rect.size());

            ui.horizontal(|ui| {
                // Load Shader
                let file_button = ui.button("File:");
                if file_button.clicked() {
                    let file_picker = rfd::FileDialog::new()
                        .set_directory(&get_config().shader_directory.canonicalize().unwrap_or("./".into()));
                    if let Some(file_path) = file_picker.pick_file() {
                        self.load_material(&file_path);
                        save_config = true;
                    }
                }
                ui.text_edit_singleline(&mut self.shader_path.to_str().unwrap());

                // Save Shader
                let save_button = ui.button("Save");
                if save_button.clicked() {
                    let file_picker = FileDialog::new()
                        .set_title("Save Material")
                        .set_directory(&get_config().shader_directory.canonicalize().unwrap())
                        .set_file_name(self.shader_path.file_name().unwrap_or(&std::ffi::OsString::from("./")).to_string_lossy())
                        .save_file();

                    if let Some(save_file_path) = file_picker {
                        self.save_material(&save_file_path);
                    } else {
                        println!("Failed to save material");
                    }
                }
            });

            // Uniforms and textures
            ui.add_space(text_height * 2.);

            ui.label("Uniforms:");
            ScrollArea::vertical()
                .id_salt("uniform_scroll")
                .max_width(usable_width)
                .max_height(75.)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.uniforms_text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10),
                    );
                });

            // Convenience buttons for adding storage buffer variables
            ui.add_space(text_height);
            ui.horizontal(|ui| {
                if ui.button("Add Vec4").clicked() {
                    if !self.uniforms_text.is_empty() {
                        self.uniforms_text += "\n";
                    }
                    self.uniforms_text += "temp_vec4_var = { type = \"vec4f\", default = [1.0, 1.0, 1.0, 1.0] }";
                }
                if ui.button("Add f32").clicked() {
                    if !self.uniforms_text.is_empty() {
                        self.uniforms_text += "\n";
                    }
                    self.uniforms_text += "temp_f32_var = { type = \"f32\", default = 1.2 }";
                }
                if ui.button("Add Add Array").clicked() {
                    if !self.uniforms_text.is_empty() {
                        self.uniforms_text += "\n";
                    }
                    self.uniforms_text += "temp_array_var = { type = \"array<vec4f, 3>\", default = [\n\t[1.0, 0.8, 0.6, 1.0],\n\t[0.5, 0.7, 0.9, 1.0],\n\t[0.1, 0.2, 0.3, 1.0],\n] }";
                }
            });

            // Textures
            ui.add_space(text_height * 2.);
            ui.label("Textures:");
            ScrollArea::vertical()
                .id_salt("texture_scroll")
                .max_width(usable_width)
                .max_height(75.)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.textures_text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10),
                    );
                });
                ui.add_space(text_height);
                if ui.button("Add Texture").clicked() {
                    if !self.textures_text.is_empty() {
                        self.textures_text += "\n";
                    }
                    self.textures_text += "temp_texture = \"linear\"";
                }

            // World Offset
            ui.add_space(text_height * 2.);
            ui.label("World Offset");
            ScrollArea::vertical()
                .id_salt("world_offset")
                .max_width(usable_width)
                .max_height(usable_height / 5.)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.world_offset_text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(25)
                            .font(egui::TextStyle::Monospace),
                    );
                });

            // Fragment Color
            ui.add_space(text_height * 2.);
            ui.label("Fragment Color");
            ScrollArea::vertical()
                .id_salt("fragment_color")
                .max_width(usable_width)
                .max_height(usable_height / 5.)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut self.frag_color_text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(25)
                            .font(egui::TextStyle::Monospace),
                    );
                });

            ui.add_space(text_height);
            let compile_button = ui.button("Compile");
            if compile_button.clicked() {
                cmd_string = format!(
                    "compile##DELIM##{}\n##DELIM##{}\n##DELIM##{}\n##DELIM##{}\n##DELIM##",
                    self.uniforms_text.replace("\r", "\n").trim_start().trim_end(),
                    self.textures_text.replace("\r", "\n").trim_start().trim_end(),
                    self.world_offset_text.replace("\r", "\n").trim_start().trim_end(),
                    self.frag_color_text.replace("\r", "\n").trim_start().trim_end(),
                );
            }

            ui.add_space(text_height * 2.);

            ui.horizontal(|ui| {
                ui.label("Textures");
                ComboBox::from_id_salt("Textures").show_ui(ui, |ui| {
                    for i in 0..16 {
                        let file_button = ui.button(format!("Texture[{i}]"));
                        if file_button.clicked() {
                            save_config = true;
                            let file_picker = rfd::FileDialog::new().set_directory(
                                &get_config().texture_directories[i].canonicalize().unwrap_or("./".into())
                            );
                            if let Some(file_path) = file_picker.pick_file() {
                                cmd_string =
                                    format!("load_texture##DELIM##{}##DELIM##", file_path.to_str().unwrap());
                                if let Some(file_name) = file_path.file_name() {
                                    self.textures[i] =
                                        file_name.to_string_lossy().to_owned().to_string();
                                }
                                get_config().texture_directories[i] = file_path
                                    .parent()
                                    .unwrap_or(PathBuf::from("./").as_path())
                                    .to_path_buf();
                            }
                        }
                        ui.text_edit_singleline(&mut self.textures[i]);
                    }
                });
            });
        });

        unsafe {
            if let Ok(mut shared_mem) = SHARED_MEM_FILE.try_lock() {
                let read_barrier = { &*(shared_mem.as_ptr() as *mut AtomicBool) };

                if read_barrier.load(Ordering::Acquire) {
                    let incoming_message =
                        std::str::from_utf8(&shared_mem[1..]).expect("Invalid UTF-8");

                    if incoming_message.as_bytes()[0] != b'\0' {
                        println!("Gui - clear!");
                        // Process incoming messages here
                        shared_mem[1..].fill(b'\0');
                    }

                    if cmd_string.len() > 0 {
                        println!(
                            "Gui - Sending command wth len {} {cmd_string}",
                            cmd_string.len()
                        );
                        shared_mem[1..cmd_string.len() + 1].copy_from_slice(cmd_string.as_bytes());
                    }

                    read_barrier.store(false, Ordering::Release);
                }

                shared_mem.flush().expect("Failed to flush");
            }
        }

        if save_config {
            let _ = fs::write(
                USER_SETTINGS_PATH,
                serde_json::to_string_pretty(&get_config()).unwrap(),
            );
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw_input: &mut egui::RawInput) {
        //  self.keypad.bump_events(ctx, raw_input);
    }
}

fn main() -> eframe::Result {
    env_logger::init();

    // Config file
    if !Path::new(USER_SETTINGS_PATH).exists() {
        let default_config = UserSettings {
            version: MATERIAL_EDITOR_VERSION,
            shader_directory: "./".into(),
            texture_directories: std::array::from_fn(|_| "./".into()),
        };
        println!("WRITING CONFIG!");
        let _ = fs::write(
            USER_SETTINGS_PATH,
            serde_json::to_string_pretty(&default_config).unwrap(),
        );
    }

    let settings = fs::read_to_string(USER_SETTINGS_PATH).expect("Failed to read config file");
    let user_settings: UserSettings =
        serde_json::from_str(&settings).expect("Failed to parse config file");

    unsafe {
        GLOBAL_CONFIG = Some(user_settings);
    }

    // Window and Gui
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 900.0])
            .with_position([800.0, 25.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Material Editor",
        options,
        Box::new(|cc| {
            // Use the dark theme
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::<MaterialEditor>::default())
        }),
    )
}
