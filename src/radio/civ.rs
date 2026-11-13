pub const RADIO_ADDRESS: u8 = 0x94;
pub const CONTROLLER_ADDRESS: u8 = 0xE0;

pub struct CivProtocol;
impl CivProtocol {
    pub fn read_frequency_command() -> [u8; 6] {
        [0xFE, 0xFE, RADIO_ADDRESS, CONTROLLER_ADDRESS, 0x03, 0xFD]
    }

    pub fn parse_frequency(frame: &[u8]) -> Result<u64, String> {
        if frame.len() != 11
            || frame[..5] != [0xFE, 0xFE, CONTROLLER_ADDRESS, RADIO_ADDRESS, 0x03]
            || frame[10] != 0xFD
        {
            return Err("Not an IC-7300 frequency response".into());
        }
        let mut hz = 0_u64;
        let mut multiplier = 1_u64;
        for &byte in &frame[5..10] {
            let low = (byte & 0x0f) as u64;
            let high = (byte >> 4) as u64;
            if low > 9 || high > 9 {
                return Err("Invalid packed BCD".into());
            }
            hz += (low + high * 10) * multiplier;
            multiplier *= 100;
        }
        Ok(hz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_14074000_hz() {
        let frame = [
            0xfe, 0xfe, 0xe0, 0x94, 0x03, 0x00, 0x40, 0x07, 0x14, 0x00, 0xfd,
        ];
        assert_eq!(CivProtocol::parse_frequency(&frame).unwrap(), 14_074_000);
    }
    #[test]
    fn rejects_wrong_address() {
        let frame = [
            0xfe, 0xfe, 0xe0, 0x95, 0x03, 0x00, 0x40, 0x07, 0x14, 0x00, 0xfd,
        ];
        assert!(CivProtocol::parse_frequency(&frame).is_err());
    }
}
