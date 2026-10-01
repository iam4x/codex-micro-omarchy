use crate::model::{Control, Phase};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const VID: u16 = 0x303a;
pub const PID: u16 = 0x8360;

pub fn reports(value: &Value) -> Result<Vec<[u8; 64]>> {
    let bytes = serde_json::to_vec(value)?;
    Ok(bytes
        .chunks(61)
        .map(|chunk| {
            let mut report = [0; 64];
            report[..3].copy_from_slice(&[6, 2, chunk.len() as u8]);
            report[3..3 + chunk.len()].copy_from_slice(chunk);
            report
        })
        .collect())
}

#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}
impl Decoder {
    pub fn feed(&mut self, report: &[u8]) -> Result<Vec<Value>> {
        if report.len() < 3 || report[0] != 6 || report[1] != 2 {
            return Ok(Vec::new());
        }
        let length = usize::from(report[2]);
        ensure!(
            length <= 61 && report.len() >= length + 3,
            "Invalid HID payload size"
        );
        self.buffer.extend_from_slice(&report[3..length + 3]);
        ensure!(
            self.buffer.len() <= 1_048_576,
            "HID receive buffer is too large"
        );
        let mut stream = serde_json::Deserializer::from_slice(&self.buffer).into_iter::<Value>();
        let mut result = Vec::new();
        for message in stream.by_ref() {
            match message {
                Ok(message) => result.push(message),
                Err(error) if error.is_eof() => break,
                Err(error) => return Err(error.into()),
            }
        }
        let consumed = stream.byte_offset();
        self.buffer.drain(..consumed);
        Ok(result)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub control: Control,
    pub phase: Phase,
}
#[derive(Default)]
pub struct InputDecoder {
    mic_switches: u8,
}
impl InputDecoder {
    pub fn notification(&mut self, message: &Value) -> Option<Input> {
        if message.get("m")?.as_str()? != "v.oai.hid" {
            return None;
        }
        let params = message.get("p")?;
        let key = params.get("k")?.as_str()?;
        let phase = match params.get("act")?.as_u64()? {
            0 => Phase::Release,
            1 => Phase::Press,
            2 => Phase::Step,
            _ => return None,
        };
        if key == "ACT10" || key == "ACT11" {
            if phase == Phase::Step {
                return None;
            }
            let old = self.mic_switches;
            let mask = if key == "ACT10" { 1 } else { 2 };
            if phase == Phase::Press {
                self.mic_switches |= mask;
            } else {
                self.mic_switches &= !mask;
            }
            return match (old == 0, self.mic_switches == 0) {
                (true, false) => Some(Input {
                    control: Control::Mic,
                    phase: Phase::Press,
                }),
                (false, true) => Some(Input {
                    control: Control::Mic,
                    phase: Phase::Release,
                }),
                _ => None,
            };
        }
        let control = Control::from_id(key)?;
        if (phase == Phase::Step) != control.is_rotation() {
            return None;
        }
        Some(Input { control, phase })
    }
}

pub fn status_request(id: u64) -> Value {
    json!({"id": id, "method": "device.status"})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_utf8_and_consecutive_messages_decode() {
        let original = json!({"text": "é".repeat(100)});
        let mut decoder = Decoder::default();
        let mut decoded = Vec::new();
        for report in reports(&original).unwrap() {
            decoded.extend(decoder.feed(&report).unwrap());
        }
        for report in reports(&json!({"id": 2})).unwrap() {
            decoded.extend(decoder.feed(&report).unwrap());
        }
        assert_eq!(decoded, [original, json!({"id": 2})]);
    }
    #[test]
    fn wide_mic_only_fires_one_press_and_release() {
        let mut decoder = InputDecoder::default();
        let result: Vec<_> = [("ACT10", 1), ("ACT11", 1), ("ACT10", 0), ("ACT11", 0)]
            .into_iter()
            .filter_map(|(key, act)| {
                decoder.notification(&json!({"m":"v.oai.hid", "p":{"k":key,"act":act}}))
            })
            .map(|input| input.phase)
            .collect();
        assert_eq!(result, [Phase::Press, Phase::Release]);
    }
    #[test]
    fn invalid_frame_and_non_vendor_reports_are_handled() {
        let mut decoder = Decoder::default();
        assert!(decoder.feed(&[1, 0, 0]).unwrap().is_empty());
        assert!(decoder.feed(&[6, 2, 62]).is_err());
    }
}
