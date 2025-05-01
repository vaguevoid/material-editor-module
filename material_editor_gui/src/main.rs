// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)]
use std::{
    env, fs,
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use eframe::egui::{self, CentralPanel, TextEdit};
use memmap2::MmapMut;
use once_cell::sync::Lazy;
use std::fs::OpenOptions;

static SHARED_MEM_FILE: Lazy<Mutex<MmapMut>> = Lazy::new(|| {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("shared_memory.bin")
        .expect("Failed to open file");

    Mutex::new(unsafe { MmapMut::map_mut(&file).expect("Failed to mmap") })
});

struct MaterialEditor {
    file_path: PathBuf,
    textures_text: String,
    uniforms_text: String,
    world_offset_text: String,
    frag_color_text: String,
}

impl MaterialEditor {}

impl Default for MaterialEditor {
    fn default() -> Self {
        let file_path: PathBuf = env::current_dir().unwrap();

        Self {
            file_path,
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

        CentralPanel::default().show(ctx, |ui| {
            let available_rect = ctx.available_rect();
            let usable_width = ui.max_rect().width() - ui.spacing().item_spacing.x;
            let text_height = 12_f32;

            ui.set_min_size(available_rect.size());

            ui.horizontal(|ui| {
                let file_button = ui.button("File: ");
                if file_button.clicked() {
                    if let Some(file_path) = rfd::FileDialog::new().pick_file() {
                        self.file_path = file_path;
                        if let Ok(material_toml) = fs::read_to_string(&self.file_path) {
                            let key = "[uniform_types]";
                            if let Some(snippet_key_idx) = material_toml.find(key) {
                                let snippet_start = snippet_key_idx + key.len();
                                let snippet = &material_toml[snippet_start..];
                                let snippet_end = snippet.find('[').unwrap_or(snippet.len());
                                self.uniforms_text = snippet[..snippet_end]
                                    .trim_start_matches(|c| c == '\n' || c == '\r')
                                    .to_string();
                            }

                            let key = "[texture_descs]";
                            if let Some(snippet_key_idx) = material_toml.find(key) {
                                let snippet_start = snippet_key_idx + key.len();
                                let snippet = &material_toml[snippet_start..];
                                let snippet_end = snippet.find('[').unwrap_or(snippet.len());
                                self.textures_text = snippet[..snippet_end]
                                    .trim_start_matches(|c| c == '\n' || c == '\r')
                                    .to_string();
                            }

                            let key = "get_world_offset";
                            if let Some(snippet_key_idx) = material_toml.find(key) {
                                let snippet = &material_toml[snippet_key_idx..];

                                let start = snippet.find("\"\"\"").unwrap();
                                let end = snippet[start + 3..].find("\"\"\"").unwrap();
                                self.world_offset_text = snippet[start + 3..start + 3 + end]
                                    .trim_start_matches(|c| c == '\n' || c == '\r')
                                    .to_string();
                            }

                            let key = "get_fragment_color";
                            if let Some(snippet_key_idx) = material_toml.find(key) {
                                let snippet = &material_toml[snippet_key_idx..];

                                let start = snippet.find("\"\"\"").unwrap();
                                let end = snippet[start + 3..].find("\"\"\"").unwrap();
                                self.frag_color_text = snippet[start + 3..start + 3 + end]
                                    .trim_start_matches(|c| c == '\n' || c == '\r')
                                    .to_string();
                            }
                        }
                    }
                }

                ui.text_edit_singleline(&mut self.file_path.to_str().unwrap());
            });

            // Uniforms
            ui.add_space(text_height * 2.);
            ui.label("Uniforms");
            ui.add_sized(
                [usable_width, 25.],
                TextEdit::multiline(&mut self.uniforms_text)
                    .code_editor()
                    .desired_rows(5)
                    .font(egui::TextStyle::Monospace),
            );

            // Textures
            ui.add_space(text_height * 2.);
            ui.label("Textures");
            ui.add_sized(
                [usable_width, 25.],
                TextEdit::multiline(&mut self.textures_text)
                    .code_editor()
                    .desired_rows(5)
                    .font(egui::TextStyle::Monospace),
            );

            ui.add_space(text_height * 2.);
            ui.label("World Offset");
            ui.add_sized(
                [usable_width, 150.],
                TextEdit::multiline(&mut self.world_offset_text)
                    .code_editor()
                    .desired_rows(10)
                    .font(egui::TextStyle::Monospace),
            );

            ui.add_space(text_height * 2.);
            ui.label("Fragment Color");
            ui.add_sized(
                [usable_width, 150.],
                TextEdit::multiline(&mut self.frag_color_text)
                    .code_editor()
                    .desired_rows(10)
                    .font(egui::TextStyle::Monospace),
            );

            ui.add_space(text_height);
            let compile_button = ui.button("Compile");
            if compile_button.clicked() {
                cmd_string = format!(
                    "compile ##delimiter## {} ##delimiter## {} ##delimiter## {} ##delimiter## {}",
                    self.uniforms_text,
                    self.textures_text,
                    self.world_offset_text,
                    self.frag_color_text
                );
            }

            ui.add_space(text_height);
            let save_button = ui.button("Save");
            if save_button.clicked() {
                println!("Save button clicked!");
            }
        });

        unsafe {
            if let Ok(mut shared_mem) = SHARED_MEM_FILE.try_lock() {
                let read_barrier = { &*(shared_mem.as_ptr() as *mut AtomicBool) };

                if read_barrier.load(Ordering::Acquire) {
                    let incoming_message =
                        std::str::from_utf8(&shared_mem[1..]).expect("Invalid UTF-8");

                    // TODO: Always true
                    if incoming_message.len() > 1 {
                        /*
                        let parts: Vec<&str> = incoming_message.split("##delimiter##").collect();

                        if parts.len() >= 3 {
                            self.world_offset_text = parts[1].to_string();
                            self.frag_color_text = parts[2].to_string();
                        }*/
                    }
                    shared_mem[1..].fill(b'\0');

                    if cmd_string.len() > 0 {
                        // println!("Gui - Sending command wth len {} {cmd_string}", cmd_string.len());
                        shared_mem[1..cmd_string.len() + 1].copy_from_slice(cmd_string.as_bytes());
                    }

                    read_barrier.store(false, Ordering::Release);
                }

                shared_mem.flush().expect("Failed to flush");
            }
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, _raw_input: &mut egui::RawInput) {
        //  self.keypad.bump_events(ctx, raw_input);
    }
}

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 800.0]).with_position([800.0, 100.0]),
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
