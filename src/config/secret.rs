use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

pub fn encode_secret(decoded_secret: &str) -> String {
    // Convert string to UTF-16 bytes
    let utf16_bytes: Vec<u8> = decoded_secret
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();

    // Apply the encoding transformation
    let encoded: Vec<u8> = utf16_bytes
        .iter()
        .map(|&b| {
            let b = !b; // Bitwise NOT
            let fsb = (b & 128) > 0; // Get first significant bit
            let b = b << 1; // Shift left by 1
            b | if fsb { 1 } else { 0 } // Set last bit based on fsb
        })
        .collect();

    // Convert to base64
    BASE64.encode(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_encoding() {
        let test_secret = "test123";
        let encoded = encode_secret(test_secret);
        assert!(!encoded.is_empty());
    }
} 