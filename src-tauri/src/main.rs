// Hides the console window on Windows release builds. Harmless on macOS.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    z_i_p_lib::run();
}
