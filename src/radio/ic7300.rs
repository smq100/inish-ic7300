use std::io::{Read, Write};
use std::time::{Duration, Instant};
use serialport::SerialPort;
use super::civ::CivProtocol;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct Ic7300 {
    port: Box<dyn SerialPort>,
    pub port_name: String,
    pub baud_rate: u32,
}

impl Ic7300 {
    pub fn list_ports() -> Result<Vec<String>> {
        let ports = serialport::available_ports()?;
        Ok(ports.into_iter().map(|p| format!("{} ({:?})", p.port_name, p.port_type)).collect())
    }

    pub fn connect(port_name: &str, baud_rate: u32) -> Result<Self> {
        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(100)).open()?;
        Ok(Self { port, port_name: port_name.into(), baud_rate })
    }

    pub fn read_frequency(&mut self) -> Result<u64> {
        self.port.write_all(&CivProtocol::read_frequency_command())?;
        self.port.flush()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut frame: Vec<u8> = Vec::new();
        let mut byte = [0u8; 1];
        while Instant::now() < deadline {
            match self.port.read(&mut byte) {
                Ok(1) => {
                    frame.push(byte[0]);
                    // Resynchronize at the next FE FE preamble, ignoring noise.
                    while frame.len() >= 2 && frame[..2] != [0xFE, 0xFE] {
                        frame.remove(0);
                    }
                    if frame.len() > 256 { frame.clear(); continue; }
                    if byte[0] == 0xFD {
                        if let Ok(hz) = CivProtocol::parse_frequency(&frame) { return Ok(hz); }
                        // Echoed command, unsolicited message, or malformed response.
                        frame.clear();
                    }
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => return Err(e.into()),
            }
        }
        Err("Timed out waiting for IC-7300 CI-V frequency response".into())
    }
}
