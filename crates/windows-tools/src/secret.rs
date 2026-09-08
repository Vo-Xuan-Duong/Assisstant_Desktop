use std::{ffi::c_void, slice};

use thiserror::Error;
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
            CryptUnprotectData,
        },
    },
    core::PCWSTR,
};

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("secret payload is too large for Windows DPAPI")]
    TooLarge,
    #[error("Windows DPAPI failed: {0}")]
    Dpapi(String),
    #[error("Windows DPAPI returned an invalid output buffer")]
    InvalidOutput,
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

#[cfg(test)]
mod tests {
    use super::*;

    // This test is intentionally Windows-only at runtime and is not run remotely
    // by project policy. It provides a local validation target for the API shape.
    #[test]
    fn dpapi_round_trip_current_user() {
        let protected = protect_for_current_user(b"assistant-secret").unwrap();
        assert_ne!(protected, b"assistant-secret");
        let clear = unprotect_for_current_user(&protected).unwrap();
        assert_eq!(clear, b"assistant-secret");
    }
}
