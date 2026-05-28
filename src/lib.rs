mod apple_helper;
mod audio;
pub mod benchmark;
mod config;
mod hotkey;
mod llm;
mod local_models;
mod model_catalog;
mod overlay;
mod paste;
mod permissions;
mod pipeline;
mod platform;
mod prewarm;
mod startup;
mod state;
mod stt;
mod trace;
mod ui;

pub fn run() {
    startup::run();
}
