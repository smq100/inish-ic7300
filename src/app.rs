use crate::analyzer::{AnalysisConfig, NoiseAnalyzer};
use crate::audio::{AudioInput, AudioService, Result};
use crate::radio::ic7300::Ic7300;
use std::io::{self, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub struct Application {
    input: Option<AudioInput>,
    analyzer: NoiseAnalyzer,
    radio: Option<Ic7300>,
}

impl Application {
    pub fn new(config: AnalysisConfig) -> Self {
        Self {
            input: None,
            analyzer: NoiseAnalyzer::new(config),
            radio: None,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            println!("\n=== IC-7300 Audio / Noise Monitor ===");
            match &self.input {
                Some(input) => println!("Connected: {} ({} Hz)", input.name, input.sample_rate),
                None => println!("Connected: none"),
            }
            println!("1. List USB/audio input devices");
            println!("2. Connect to input device");
            println!("3. Live noise-floor monitor");
            println!("4. Disconnect audio device");
            println!("5. Measurement settings");
            println!("6. List serial/CAT ports");
            println!("7. Connect to IC-7300 CAT");
            println!("8. Test CAT (read frequency)");
            println!("9. Disconnect CAT");
            if let Some(r) = &self.radio {
                println!("CAT: {} @ {} baud", r.port_name, r.baud_rate);
            }
            println!("0. Exit");
            match prompt("Select: ")?.as_str() {
                "1" => self.list_devices(),
                "2" => self.connect(),
                "3" => self.live(),
                "4" => {
                    self.input = None;
                    println!("Disconnected.");
                }
                "5" => self.settings(),
                "6" => self.list_serial_ports(),
                "7" => self.connect_cat_interactive(),
                "8" => self.test_cat(),
                "9" => {
                    self.radio = None;
                    println!("CAT disconnected.");
                }
                "0" => break,
                _ => println!("Invalid selection."),
            }
        }
        Ok(())
    }

    fn list_serial_ports(&self) {
        match Ic7300::list_ports() {
            Ok(ports) if ports.is_empty() => println!("No serial ports found."),
            Ok(ports) => {
                for port in ports {
                    println!("{port}");
                }
            }
            Err(e) => eprintln!("Serial enumeration failed: {e}"),
        }
    }

    fn connect_cat_interactive(&mut self) {
        self.list_serial_ports();
        let Ok(port_name) = prompt("Port name (e.g. COM3 or /dev/ttyUSB0): ") else {
            return;
        };
        if port_name.is_empty() {
            println!("Port required.");
            return;
        }
        let Ok(baud_text) = prompt("Baud rate [115200]: ") else {
            return;
        };
        let baud_rate = if baud_text.is_empty() {
            115200
        } else {
            match baud_text.parse::<u32>() {
                Ok(rate) if rate > 0 => rate,
                _ => {
                    println!("Invalid baud rate.");
                    return;
                }
            }
        };
        match self.connect_cat_port(&port_name, baud_rate) {
            Ok(()) => println!("CAT serial port opened."),
            Err(e) => eprintln!("CAT connection failed: {e}"),
        }
    }

    pub fn connect_cat_port(&mut self, port_name: &str, baud_rate: u32) -> Result<()> {
        let radio = Ic7300::connect(port_name, baud_rate)?;
        self.radio = Some(radio);
        Ok(())
    }

    fn test_cat(&mut self) {
        let Some(radio) = self.radio.as_mut() else {
            println!("Connect to the radio CAT port first.");
            return;
        };
        match radio.read_frequency() {
            Ok(hz) => println!("CAT OK: {:.6} MHz ({hz} Hz)", hz as f64 / 1_000_000.0),
            Err(e) => eprintln!("CAT test failed: {e}"),
        }
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
        match self.connect_device(index) {
            Ok(()) => {
                if let Some(input) = &self.input {
                    println!("Connected to {} at {} Hz", input.name, input.sample_rate);
                }
            }
            Err(e) => eprintln!("Connection failed: {e}"),
        }
    }

    pub fn connect_device(&mut self, index: usize) -> Result<()> {
        let input = AudioService::connect(index)?;
        self.input = Some(input);
        Ok(())
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

    pub fn live(&self) {
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
