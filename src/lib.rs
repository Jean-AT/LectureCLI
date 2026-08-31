pub mod app;
pub mod capture;
pub mod cli;
pub mod platform;
pub mod session;
pub mod signals;
pub mod transcribe;
pub mod util;
pub mod vad;
pub mod wav;

pub fn run() -> anyhow::Result<()> {
    cli::run()
}
