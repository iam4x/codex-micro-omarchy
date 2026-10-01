use anyhow::{Context, Result, ensure};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{BufRead, Read, Write};

const MAX_FRAME_BYTES: usize = 1_048_576;

pub fn read<T: DeserializeOwned>(reader: &mut impl BufRead) -> Result<T> {
    let mut frame = Vec::new();
    reader
        .take((MAX_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut frame)?;
    ensure!(
        frame.len() <= MAX_FRAME_BYTES,
        "Socket message is too large"
    );
    ensure!(frame.last() == Some(&b'\n'), "Incomplete socket message");
    serde_json::from_slice(&frame).context("Invalid socket message")
}

pub fn write(writer: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let mut frame = serde_json::to_vec(value)?;
    ensure!(frame.len() < MAX_FRAME_BYTES, "Socket message is too large");
    frame.push(b'\n');
    writer.write_all(&frame)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::Cursor;

    #[test]
    fn preserves_multiline_unicode_and_consecutive_frames() {
        let original = json!({"text": "Line 1\n\nCafé".repeat(10_000)});
        let mut bytes = Vec::new();
        write(&mut bytes, &original).unwrap();
        write(&mut bytes, &json!({"ok":true})).unwrap();
        let mut reader = Cursor::new(bytes);
        assert_eq!(read::<Value>(&mut reader).unwrap(), original);
        assert_eq!(read::<Value>(&mut reader).unwrap(), json!({"ok":true}));
    }

    #[test]
    fn rejects_truncated_invalid_and_oversized_frames() {
        for frame in [b"".as_slice(), b"{}", b"{broken}\n"] {
            assert!(read::<Value>(&mut Cursor::new(frame)).is_err());
        }
        let oversized = vec![b' '; MAX_FRAME_BYTES + 1];
        assert!(read::<Value>(&mut Cursor::new(oversized)).is_err());
        assert!(write(&mut Vec::new(), &"x".repeat(MAX_FRAME_BYTES)).is_err());
    }
}
