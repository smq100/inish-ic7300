mod analyzer;
mod app;
mod audio;
mod radio;

use analyzer::AnalysisConfig;
use app::Application;
use audio::AudioService;
use clap::Parser;
use radio::ic7300::Ic7300;
use std::error::Error;

fn main() {
    if let Err(e) = run() {
        eprintln!("Application error: {e}");
        std::process::exit(1);
    }
}

#[derive(Parser)]
#[command(version, about = "IC-7300 audio and noise monitor")]
struct Args {
    /// Connect to this zero-based audio input device index at startup
    #[arg(long, value_name = "INDEX")]
    device_index: Option<usize>,

    /// Connect to this radio CAT serial port at startup
    #[arg(long, value_name = "PORT")]
    cat_port: Option<String>,

    /// CAT serial baud rate
    #[arg(long, default_value_t = 115_200)]
    baud_rate: u32,

    /// Lower edge of the analysis band in Hz
    #[arg(long, value_name = "HZ")]
    low_hz: Option<f64>,

    /// Upper edge of the analysis band in Hz
    #[arg(long, value_name = "HZ")]
    high_hz: Option<f64>,

    /// Rolling analysis window in seconds (1-10)
    #[arg(long, value_name = "SECONDS")]
    window_seconds: Option<usize>,

    /// FFT size (at least 3 samples)
    #[arg(long, value_name = "SAMPLES")]
    fft_size: Option<usize>,

    /// Start live monitoring immediately; requires --device-index
    #[arg(long, requires = "device_index")]
    monitor: bool,

    /// Print available audio input devices and exit unless another startup action is requested
    #[arg(long)]
    list_devices: bool,

    /// Print available serial ports and exit unless another startup action is requested
    #[arg(long)]
    list_serial_ports: bool,
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut config = AnalysisConfig::default();
    if let Some(low_hz) = args.low_hz {
        config.low_hz = low_hz;
    }
    if let Some(high_hz) = args.high_hz {
        config.high_hz = high_hz;
    }
    if let Some(window_seconds) = args.window_seconds {
        config.window_seconds = window_seconds;
    }
    if let Some(fft_size) = args.fft_size {
        config.fft_size = fft_size;
    }

    if !config.low_hz.is_finite()
        || !config.high_hz.is_finite()
        || config.low_hz < 0.0
        || config.high_hz <= config.low_hz
        || !(1..=10).contains(&config.window_seconds)
        || config.fft_size < 3
    {
        return Err("Invalid analysis settings; require 0 <= low-hz < high-hz, window-seconds 1-10, and fft-size >= 3".into());
    }
    if args.baud_rate == 0 {
        return Err("Baud rate must be greater than zero".into());
    }

    if args.list_devices {
        for (index, name) in AudioService::list_devices()? {
            println!("[{index}] {name}");
        }
    }
    if args.list_serial_ports {
        for port in Ic7300::list_ports()? {
            println!("{port}");
        }
    }

    let has_startup_action = args.device_index.is_some() || args.cat_port.is_some() || args.monitor;
    if (args.list_devices || args.list_serial_ports) && !has_startup_action {
        return Ok(());
    }

    let mut app = Application::new(config);
    if let Some(index) = args.device_index {
        app.connect_device(index)?;
    }
    if let Some(port) = args.cat_port.as_deref() {
        app.connect_cat_port(port, args.baud_rate)?;
    }
    if args.monitor {
        app.live();
        return Ok(());
    }
    app.run()?;
    Ok(())
}
