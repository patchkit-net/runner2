use crate::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek};
use log::{debug, error};

pub mod secret;

const MAGIC_BYTES: [u8; 4] = [46, 98, 76, 97]; // ".bLa"

#[derive(Debug, Deserialize, Serialize)]
pub struct LauncherData {
    pub patcher_secret: String,
    pub app_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_identifier: Option<String>,
}

impl LauncherData {
    pub fn from_binary<R: Read + Seek>(mut reader: R) -> Result<Self> {
        debug!("Reading binary DAT file");
        let patcher_secret = read_encoded_string(&mut reader)?;
        debug!("Read patcher_secret: {}", patcher_secret);
        let app_secret = read_encoded_string(&mut reader)?;
        debug!("Read app_secret: {}", app_secret);
        
        Ok(Self {
            patcher_secret,
            app_secret,
            app_display_name: None,
            app_author: None,
            app_identifier: None,
        })
    }

    pub fn from_json<R: Read + Seek>(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        
        if magic != MAGIC_BYTES {
            return Err(crate::Error::DatFile("Invalid magic bytes".into()));
        }

        let json_str = read_encoded_string(&mut reader)?;
        Ok(serde_json::from_str(&json_str)?)
    }
}

fn read_encoded_string<R: Read + Seek>(mut reader: R) -> Result<String> {
    let length = reader.read_u32::<LittleEndian>()?;
    debug!("String length: {}", length);
    let mut encoded_bytes = vec![0u8; length as usize];
    reader.read_exact(&mut encoded_bytes)?;
    
    let decoded_bytes = decode_byte_array(&encoded_bytes);
    debug!("Decoded bytes length: {}", decoded_bytes.len());
    if decoded_bytes.is_empty() {
        return Err(crate::Error::DatFile("Decoded string is empty".into()));
    }
    
    String::from_utf8(decoded_bytes)
        .map_err(|e| {
            error!("UTF-8 decoding error: {}", e);
            error!("Raw decoded bytes: {:?}", e.as_bytes());
            crate::Error::DatFile(format!("Invalid UTF-8: {}", e))
        })
}

fn decode_byte_array(encoded_bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(encoded_bytes.len() / 2);
    let mut i = 0;
    
    while i < encoded_bytes.len() {
        if i + 1 < encoded_bytes.len() {
            let b = encoded_bytes[i];
            // Skip the zero byte
            // i + 1 is the zero byte
            
            // First get the LSB which was originally MSB
            let lsb = b & 1;
            // Shift right by 1 and restore original MSB
            let mut decoded = b >> 1;
            decoded = decoded | (lsb << 7);
            // Invert all bits to complete the decoding
            decoded = !decoded;
            
            result.push(decoded);
        }
        i += 2;
    }
    
    debug!("Decoding {} bytes -> {} bytes", encoded_bytes.len(), result.len());
    if !result.is_empty() {
        debug!("First decoded byte: {:08b}", result[0]);
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn encode_byte(b: u8) -> u8 {
        // First invert all bits
        let inverted = !b;
        // Extract LSB and move it to MSB
        let lsb = inverted & 1;
        // Shift right by 1 and set LSB to original MSB
        let shifted = (inverted >> 1) | (lsb << 7);
        shifted
    }

    #[test]
    fn test_decode_byte_array() {
        let test_str = b"test";
        let mut input = Vec::new();
        for &b in test_str {
            input.push(encode_byte(b));
            input.push(0);
        }
        let decoded = decode_byte_array(&input);
        assert_eq!(decoded, b"test");
    }

    #[test]
    fn test_read_encoded_string() {
        let test_str = b"test";
        let mut data = Vec::new();
        
        // Length: number of bytes * 2 (each byte is followed by a zero)
        data.extend_from_slice(&((test_str.len() * 2) as u32).to_le_bytes());
        
        // Encode test string
        for &b in test_str {
            data.push(encode_byte(b));
            data.push(0);
        }
        
        let cursor = Cursor::new(data);
        let result = read_encoded_string(cursor).unwrap();
        assert_eq!(result, "test");
    }
} 