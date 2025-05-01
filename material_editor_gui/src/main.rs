// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)]
use std::{
    env,
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
        .create(true)
        .open("shared_memory.bin")
        .expect("Failed to open file");

    file.set_len(4096).expect("Failed to set file size");

    Mutex::new(unsafe { MmapMut::map_mut(&file).expect("Failed to mmap") })
});

struct MaterialEditor {
    file_path: PathBuf,
    world_offset_text: String,
    frag_color_text: String,
}

impl MaterialEditor {}

impl Default for MaterialEditor {
    fn default() -> Self {
        let file_path: PathBuf = env::current_dir().unwrap();

        Self {
            file_path,
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
                        cmd_string = format!("load_toml {}", self.file_path.to_str().unwrap());
                    }
                }

                ui.text_edit_singleline(&mut self.file_path.to_str().unwrap());
            });

            ui.add_space(text_height * 2.);
            ui.label("World Offset");

            ui.add_sized(
                [usable_width, 150.],
                TextEdit::multiline(&mut self.world_offset_text)
                    .code_editor()
                    .desired_rows(20)
                    .font(egui::TextStyle::Monospace),
            );

            ui.add_space(text_height * 2.);
            ui.label("Fragment Color");
            ui.add_sized(
                [usable_width, 150.],
                TextEdit::multiline(&mut self.frag_color_text)
                    .code_editor()
                    .desired_rows(20)
                    .font(egui::TextStyle::Monospace),
            );

            ui.add_space(text_height);
            let compile_button = ui.button("Compile");
            if compile_button.clicked() {
                cmd_string = format!("compile");
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

                    if incoming_message.len() > 1 {
                        // TODO: Always true
                        let parts: Vec<&str> = incoming_message.split("##delimiter##").collect();

                        if parts.len() >= 3 {
                            self.world_offset_text = parts[1].to_string();
                            self.frag_color_text = parts[2].to_string();
                        }
                        /*              println!("GUI - Incoming msg {} len = {}", incoming_message, incoming_message.len());
                        for (i, part) in parts.iter().enumerate() {
                            print!("    {i}: {part}");
                        }*/
                    }
                    shared_mem[1..].fill(b'\0');

                    if cmd_string.len() > 0 {
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
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
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
