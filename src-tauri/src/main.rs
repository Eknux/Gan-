// Windows'ta release derlemesinde ek konsol penceresi açılmasını engelle.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    gan_app_lib::run()
}
