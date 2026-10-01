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
    joystick: Option<Control>,
}
impl InputDecoder {
    pub fn notification(&mut self, message: &Value) -> Vec<Input> {
        if message.get("m").and_then(Value::as_str) == Some("v.oai.rad") {
            return self.joystick_notification(message);
        }
        self.key_notification(message).into_iter().collect()
    }

    fn joystick_notification(&mut self, message: &Value) -> Vec<Input> {
        let Some(angle) = message["p"]["a"].as_f64() else {
            return Vec::new();
        };
        let Some(distance) = message["p"]["d"].as_f64() else {
            return Vec::new();
        };
        if !(0.0..=1.0).contains(&angle) || !(0.0..=1.0).contains(&distance) {
            return Vec::new();
        }
        // A smaller release threshold keeps noise near center from retriggering actions.
        let threshold = if self.joystick.is_some() { 0.15 } else { 0.25 };
        let next = if distance < threshold {
            None
        } else {
            let sector = ((angle * 4.0 + 0.5).floor() as usize) % 4;
            Some(
                [
                    Control::JoystickRight,
                    Control::JoystickDown,
                    Control::JoystickLeft,
                    Control::JoystickUp,
                ][sector],
            )
        };
        if next == self.joystick {
            return Vec::new();
        }
        let mut events = Vec::with_capacity(2);
        if let Some(control) = self.joystick {
            events.push(Input {
                control,
                phase: Phase::Release,
            });
        }
        if let Some(control) = next {
            events.push(Input {
                control,
                phase: Phase::Press,
            });
        }
        self.joystick = next;
        events
    }

    fn key_notification(&mut self, message: &Value) -> Option<Input> {
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
            .flat_map(|(key, act)| {
                decoder.notification(&json!({"m":"v.oai.hid", "p":{"k":key,"act":act}}))
            })
            .map(|input| input.phase)
            .collect();
        assert_eq!(result, [Phase::Press, Phase::Release]);
    }
    fn radial(decoder: &mut InputDecoder, angle: f64, distance: f64) -> Vec<(Control, Phase)> {
        decoder
            .notification(&json!({"m":"v.oai.rad", "p":{"a":angle,"d":distance}}))
            .into_iter()
            .map(|input| (input.control, input.phase))
            .collect()
    }

    #[test]
    fn captured_joystick_directions_press_once_and_release_at_center() {
        let mut decoder = InputDecoder::default();
        for (angle, control) in [
            (0.756254, Control::JoystickUp),
            (0.007961, Control::JoystickRight),
            (0.244134, Control::JoystickDown),
            (0.492427, Control::JoystickLeft),
        ] {
            assert_eq!(radial(&mut decoder, angle, 1.0), [(control, Phase::Press)]);
            assert!(radial(&mut decoder, angle, 0.7).is_empty());
            assert_eq!(radial(&mut decoder, 0.0, 0.0), [(control, Phase::Release)]);
            assert!(radial(&mut decoder, 0.0, 0.0).is_empty());
        }
    }

    #[test]
    fn joystick_switches_release_before_press_and_wrap_at_zero() {
        let mut decoder = InputDecoder::default();
        assert_eq!(
            radial(&mut decoder, 0.99, 1.0),
            [(Control::JoystickRight, Phase::Press)]
        );
        assert!(radial(&mut decoder, 0.01, 1.0).is_empty());
        assert_eq!(
            radial(&mut decoder, 0.25, 1.0),
            [
                (Control::JoystickRight, Phase::Release),
                (Control::JoystickDown, Phase::Press),
            ]
        );
    }

    #[test]
    fn joystick_dead_zone_and_invalid_reports_preserve_state() {
        let mut decoder = InputDecoder::default();
        assert!(radial(&mut decoder, 0.0, 0.2).is_empty());
        assert_eq!(
            radial(&mut decoder, 1.0, 0.3),
            [(Control::JoystickRight, Phase::Press)]
        );
        assert!(radial(&mut decoder, 0.0, 0.2).is_empty());
        for (angle, distance) in [(-0.1, 1.0), (1.1, 1.0), (0.0, -0.1), (0.0, 1.1)] {
            assert!(radial(&mut decoder, angle, distance).is_empty());
        }
        assert!(
            decoder
                .notification(&json!({"m":"v.oai.rad", "p":{"a":"bad","d":1}}))
                .is_empty()
        );
        assert!(
            decoder
                .notification(&json!({"m":"v.oai.rad", "p":{"a":0}}))
                .is_empty()
        );
        assert_eq!(
            radial(&mut decoder, 0.0, 0.1),
            [(Control::JoystickRight, Phase::Release)]
        );
    }

    #[test]
    fn invalid_frame_and_non_vendor_reports_are_handled() {
        let mut decoder = Decoder::default();
        assert!(decoder.feed(&[1, 0, 0]).unwrap().is_empty());
        assert!(decoder.feed(&[6, 2, 62]).is_err());
    }
}
