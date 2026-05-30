mod actions;
mod apple_helper;
mod audio;
mod config;
mod hotkey;
mod llm;
mod local_models;
mod menu;
mod model_catalog;
mod overlay;
mod paste;
mod permissions;
mod pipeline;
mod platform;
mod prewarm;
mod profile;
mod startup;
mod state;
mod stt;
mod trace;
mod ui;

// Benchmark seperate bc I don't like bloat...
#[cfg(feature = "benchmark-support")]
pub mod benchmark_support;

pub fn run() {
    startup::run();
}
