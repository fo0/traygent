#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;
mod desktop_agent_updates;
mod desktop_agents;
mod desktop_github;
mod desktop_startup;
mod desktop_updates;
mod tray_popup;

fn main() {
    desktop::run();
}
