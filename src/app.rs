use crate::analyzer::{AnalysisConfig, NoiseAnalyzer};
use crate::audio::{AudioInput, AudioService, Result};
use std::io::{self, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub struct Application {
    input: Option<AudioInput>,
    analyzer: NoiseAnalyzer,
}

impl Application {
    pub fn new() -> Self {
        Self {
            input: None,
            analyzer: NoiseAnalyzer::new(AnalysisConfig::default()),
        }
    }

    #[allow(clippy::println_empty_string)]
    pub fn run(&mut self) -> Result<()> {
        loop {
            println!("\n=== IC-7300 Audio / Noise Monitor ===\n");
            match &self.input {
                Some(input) => println!("Connected: {} ({} Hz)", input.name, input.sample_rate),
                None => println!("Connected: none"),
            }
            println!("");
            println!("1. List USB/audio input devices");
            println!("2. Connect to input device");
            println!("3. Live noise-floor monitor");
            println!("4. Disconnect audio device");
            println!("5. Measurement settings");
            println!("x. Exit");
            match prompt("\nSelect: ")?.as_str() {
                "1" => self.list_devices(),
                "2" => self.connect(),
                "3" => self.live(),
                "4" => {
                    self.input = None;
                    println!("Disconnected.");
                }
                "5" => self.settings(),
                "x" => break,
                _ => println!("Invalid selection."),
            }
        }
        Ok(())
    }

    fn list_devices(&self) {
        match AudioService::list_devices() {
            Ok(devices) if devices.is_empty() => println!("No audio input devices found."),
            Ok(devices) => {
                for (index, name) in devices {
                    println!("[{index}] {name}");
                }
            }
            Err(e) => eprintln!("Could not list devices: {e}"),
        }
    }

    fn connect(&mut self) {
        self.list_devices();
        let Ok(value) = prompt("Device index: ") else {
            return;
        };
        let Ok(index) = value.parse::<usize>() else {
            println!("Invalid index.");
            return;
        };
        // Release old stream before opening the replacement.
        self.input = None;
        match AudioService::connect(index) {
            Ok(input) => {
                println!("Connected to {} at {} Hz", input.name, input.sample_rate);
                self.input = Some(input);
            }
            Err(e) => eprintln!("Connection failed: {e}"),
        }
    }

    fn settings(&mut self) {
        let c = self.analyzer.config;
        println!(
            "Current: {:.0}-{:.0} Hz, rolling {} seconds, FFT {}",
            c.low_hz, c.high_hz, c.window_seconds, c.fft_size
        );
        println!("Press Enter to keep each value.");
        let Ok(low) = prompt("Low frequency (Hz): ") else {
            return;
        };
        let Ok(high) = prompt("High frequency (Hz): ") else {
            return;
        };
        let Ok(window) = prompt("Rolling window (seconds, 1-10): ") else {
            return;
        };
        let candidate = AnalysisConfig {
            low_hz: low.parse().unwrap_or(c.low_hz),
            high_hz: high.parse().unwrap_or(c.high_hz),
            window_seconds: window.parse().unwrap_or(c.window_seconds),
            fft_size: c.fft_size,
        };
        if candidate.low_hz < 0.0
            || candidate.high_hz <= candidate.low_hz
            || !candidate.high_hz.is_finite()
            || !candidate.low_hz.is_finite()
            || !(1..=10).contains(&candidate.window_seconds)
        {
            println!("Invalid settings; unchanged.");
        } else {
            self.analyzer.config = candidate;
            println!("Settings updated.");
        }
    }

    fn live(&self) {
        let Some(input) = &self.input else {
            println!("Connect to an input device first.");
            return;
        };
        if self.analyzer.config.high_hz >= input.sample_rate as f64 / 2.0 {
            println!(
                "High frequency must be below Nyquist ({} Hz).",
                input.sample_rate / 2
            );
            return;
        }
        println!("\nMonitoring {}. Press Enter to stop.", input.name);
        println!(
            "Band: {:.0}-{:.0} Hz | rolling {} s",
            self.analyzer.config.low_hz,
            self.analyzer.config.high_hz,
            self.analyzer.config.window_seconds
        );
        println!(
            "{:>8} {:>17} {:>21} {:>15}",
            "Elapsed", "Band dBFS", "Median dBFS/Hz", "Peak dBFS"
        );
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut s = String::new();
            let _ = io::stdin().read_line(&mut s);
            let _ = tx.send(());
        });
        let start = Instant::now();
        loop {
            if rx.try_recv().is_ok() {
                break;
            }
            thread::sleep(Duration::from_secs(1));
            let snapshot = input.snapshot(self.analyzer.config.window_seconds);
            match self.analyzer.analyze(&snapshot, input.sample_rate) {
                Some(m) => println!(
                    "{:>7.0}s {:>17.2} {:>21.2} {:>15.2}",
                    start.elapsed().as_secs_f64(),
                    m.band_power_dbfs,
                    m.median_psd_dbfs_hz,
                    m.peak_dbfs
                ),
                None => println!("Waiting for sufficient audio samples..."),
            }
        }
    }
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
