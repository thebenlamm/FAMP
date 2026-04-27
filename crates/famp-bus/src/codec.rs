//! Length-prefixed canonical-JSON frame codec. Sync, no async, no `bytes::Buf`.
//! BUS-06: 4-byte big-endian unsigned length prefix; max 16 MiB; min 1 byte payload.

use serde::{de::DeserializeOwned, Serialize};

pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const LEN_PREFIX_BYTES: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame length {0} exceeds maximum 16 MiB")]
    FrameTooLarge(u32),
    #[error("frame length cannot be zero")]
    EmptyFrame,
    #[error("canonical JSON encode failed: {0}")]
    Encode(#[from] famp_canonical::CanonicalError),
    #[error("canonical JSON decode (strict-parse) failed: {0}")]
    Decode(famp_canonical::CanonicalError),
}

pub fn encode_frame<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, FrameError> {
    let body = famp_canonical::canonicalize(value)?;
    let Ok(len) = u32::try_from(body.len()) else {
        return Err(FrameError::FrameTooLarge(u32::MAX));
    };
    if body.len() > MAX_FRAME_BYTES {
        return Err(FrameError::FrameTooLarge(len));
    }
    let mut out = Vec::with_capacity(LEN_PREFIX_BYTES + body.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn try_decode_frame<T: DeserializeOwned>(buf: &[u8]) -> Result<Option<(T, usize)>, FrameError> {
    if buf.len() < LEN_PREFIX_BYTES {
        return Ok(None);
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if len == 0 {
        return Err(FrameError::EmptyFrame);
    }
    if (len as usize) > MAX_FRAME_BYTES {
        return Err(FrameError::FrameTooLarge(len));
    }
    let total = LEN_PREFIX_BYTES + len as usize;
    if buf.len() < total {
        return Ok(None);
    }
    let payload = &buf[LEN_PREFIX_BYTES..total];
    let value: T = famp_canonical::from_slice_strict(payload).map_err(FrameError::Decode)?;
    Ok(Some((value, total)))
}
