// Tauri entry point — delegates everything to lib.rs (required by Tauri 2 conventions).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    muniani_gui_lib::run();
}