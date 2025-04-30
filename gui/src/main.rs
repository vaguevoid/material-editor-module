// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)]
use std::{
    env,
    path::{Path, PathBuf},
};

// it's an example
use eframe::egui::{self, TextEdit};
use memmap2::{MmapMut, MmapOptions};
use std::fs::{File, OpenOptions};
use std::io::Write;

mod keypad;
use keypad::Keypad;

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    // Create or open a file for shared memory
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("shared_memory.bin")
        .expect("Failed to open file");
    file.set_len(4096).expect("Failed to resize file");
    let mut mmap = unsafe { MmapMut::map_mut(&file).expect("Failed to mmap") };
    mmap[..5].copy_from_slice(b"Hello");
    mmap.flush().expect("Failed to flush");

    eframe::run_native(
        "Custom Keypad App",
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

struct MaterialEditor {
    file_path: PathBuf,
    age: u32,
    keypad: Keypad,
}

impl MaterialEditor {}

impl Default for MaterialEditor {
    fn default() -> Self {
        let file_path: PathBuf = env::current_dir().unwrap();

        Self {
            file_path,
            age: 42,
            keypad: Keypad::new(),
        }
    }
}

impl eframe::App for MaterialEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Custom Keypad")
            .default_pos([100.0, 100.0])
            .title_bar(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let file_button = ui.button("File: ");
                    if file_button.clicked() {
                        self.file_path = rfd::FileDialog::new().pick_file().unwrap_or(".\\".into())
                    }

                    ui.text_edit_singleline(&mut self.file_path.to_str().unwrap());
                });
                ui.add(
                    TextEdit::multiline(&mut "".to_string())
                        .code_editor() // Optimizes for code input
                        .desired_rows(10) // Sets the initial height
                        .font(egui::TextStyle::Monospace), // Uses monospace font for better readability
                );

                /* ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
                if ui.button("Increment").clicked() {
                    self.age += 1;
                }
                ui.label(format!(
                    "Hello '{}', age {}",
                    &mut self.file_path.to_str().unwrap_or(".\""),
                    self.age
                ));*/
            });

        self.keypad.show(ctx);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.keypad.bump_events(ctx, raw_input);
    }
}
