#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if !inzoi_better_mod_manager_lib::apply_from_cli() {
        inzoi_better_mod_manager_lib::run()
    }
}
