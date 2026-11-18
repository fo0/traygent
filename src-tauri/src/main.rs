#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;
mod desktop_agent_updates;
mod desktop_agents;
mod desktop_github;
mod desktop_profiles;
mod desktop_startup;
mod desktop_updates;
#[cfg(windows)]
mod docker_credential;
mod tray_popup;

fn main() {
    #[cfg(windows)]
    if let Some(code) = docker_credential::handle() {
        std::process::exit(code);
    }
    desktop::run();
}
