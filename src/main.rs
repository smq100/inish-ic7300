mod analyzer;
mod app;
mod audio;

fn main() {
    if let Err(e) = app::Application::new().run() {
        eprintln!("Application error: {e}");
    }
}
