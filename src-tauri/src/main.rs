#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if lazyaicode_lib::answer_askpass() {
        return;
    }
    lazyaicode_lib::run()
}
