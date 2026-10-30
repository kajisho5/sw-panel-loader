// リリース版ではWindowsで黒い窓を出さない
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = sw_panel_loader_lib::maybe_run_helper() {
        std::process::exit(code);
    }
    sw_panel_loader_lib::run()
}
