use std::{ffi::c_void, slice};

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    },
    core::PCWSTR,
};

pub const DPAPI_TEXT_PREFIX: &str = "dpapi:";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("secret payload is too large for Windows DPAPI")]
    TooLarge,
    #[error("Windows DPAPI failed: {0}")]
    Dpapi(String),
    #[error("Windows DPAPI returned an invalid output buffer")]
    InvalidOutput,
    #[error("DPAPI text envelope has invalid hexadecimal data")]
    InvalidEncoding,
    #[error("decrypted DPAPI text is not valid UTF-8")]
    InvalidUtf8,
}

pub type SecretResult<T> = Result<T, SecretError>;

/// Protect bytes with Windows DPAPI using the current user scope.
///
/// No `CRYPTPROTECT_LOCAL_MACHINE` flag is used, so another Windows user on the
/// same machine should not be able to decrypt this blob. UI is forbidden because
/// desktop startup and CLI pairing must remain non-interactive at the DPAPI layer.
pub fn protect_for_current_user(plaintext: &[u8]) -> SecretResult<Vec<u8>> {
    transform(plaintext, true)
}

/// Decrypt a blob previously produced by `protect_for_current_user`.
pub fn unprotect_for_current_user(ciphertext: &[u8]) -> SecretResult<Vec<u8>> {
    transform(ciphertext, false)
}

/// Persist a UTF-8 secret as a recognizable DPAPI envelope without pulling a
/// separate base64/hex crate into callers.
pub fn protect_text_for_current_user(plaintext: &str) -> SecretResult<String> {
    let ciphertext = protect_for_current_user(plaintext.as_bytes())?;
    Ok(format!("{DPAPI_TEXT_PREFIX}{}", hex_encode(&ciphertext)))
}

/// Decrypt a `dpapi:<hex>` text envelope.
pub fn unprotect_text_for_current_user(envelope: &str) -> SecretResult<String> {
    let encoded = envelope
        .strip_prefix(DPAPI_TEXT_PREFIX)
        .ok_or(SecretError::InvalidEncoding)?;
    let ciphertext = hex_decode(encoded)?;
    let plaintext = unprotect_for_current_user(&ciphertext)?;
    String::from_utf8(plaintext).map_err(|_| SecretError::InvalidUtf8)
}

pub fn is_dpapi_text(envelope: &str) -> bool {
    envelope.starts_with(DPAPI_TEXT_PREFIX)
}

fn transform(input: &[u8], protect: bool) -> SecretResult<Vec<u8>> {
    let input_len = u32::try_from(input.len()).map_err(|_| SecretError::TooLarge)?;
    let input_blob = CRYPT_INTEGER_BLOB {
        cbData: input_len,
        pbData: input.as_ptr().cast_mut(),
    };
    let mut output_blob = CRYPT_INTEGER_BLOB::default();

    let result = unsafe {
        if protect {
            CryptProtectData(
                &input_blob,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        } else {
            CryptUnprotectData(
                &input_blob,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output_blob,
            )
        }
    };

    result.map_err(|error| SecretError::Dpapi(error.to_string()))?;
    copy_and_free(output_blob)
}

fn copy_and_free(blob: CRYPT_INTEGER_BLOB) -> SecretResult<Vec<u8>> {
    if blob.cbData > 0 && blob.pbData.is_null() {
        return Err(SecretError::InvalidOutput);
    }

    let bytes = if blob.cbData == 0 {
        Vec::new()
    } else {
        // SAFETY: successful DPAPI calls return a buffer of exactly cbData bytes
        // allocated with LocalAlloc. We copy it before releasing it below.
        unsafe { slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec()
    };

    if !blob.pbData.is_null() {
        // DPAPI documentation requires LocalFree for pDataOut.pbData.
        let remaining = unsafe { LocalFree(Some(HLOCAL(blob.pbData.cast::<c_void>()))) };
        if !remaining.is_invalid() {
            return Err(SecretError::Dpapi(
                "LocalFree did not release the DPAPI output buffer".into(),
            ));
        }
    }

    Ok(bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(TABLE[(byte >> 4) as usize] as char);
        output.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_decode(value: &str) -> SecretResult<Vec<u8>> {
    if value.len() % 2 != 0 {
        return Err(SecretError::InvalidEncoding);
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0]).ok_or(SecretError::InvalidEncoding)?;
        let low = hex_nibble(pair[1]).ok_or(SecretError::InvalidEncoding)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip_is_strict() {
        let bytes = b"\x00\x01\xfe\xff";
        assert_eq!(hex_decode(&hex_encode(bytes)).unwrap(), bytes);
        assert!(hex_decode("abc").is_err());
        assert!(hex_decode("zz").is_err());
    }

    // This test is intentionally Windows-only at runtime and is not run remotely
    // by project policy. It provides a local validation target for the API shape.
    #[test]
    fn dpapi_round_trip_current_user() {
        let protected = protect_text_for_current_user("assistant-secret").unwrap();
        assert!(protected.starts_with(DPAPI_TEXT_PREFIX));
        assert!(!protected.contains("assistant-secret"));
        let clear = unprotect_text_for_current_user(&protected).unwrap();
        assert_eq!(clear, "assistant-secret");
    }
}
