#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod runtime_logging;

fn main() {
    runtime_logging::init();
    assisstant_desktop_lib::run();
}
