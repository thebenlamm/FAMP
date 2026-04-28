//! Async length-prefixed canonical-JSON frame codec.
//!
//! Wraps the synchronous `famp_bus::codec::{encode_frame, try_decode_frame}`
//! with `tokio::io::AsyncReadExt` / `AsyncWriteExt` so `BusClient` can
//! drive it on a `UnixStream`. The wire shape is identical to the
//! existing sync codec (BUS-06): 4-byte big-endian unsigned length
//! prefix; max 16 MiB; min 1 byte payload.

use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::BusClientError;

/// Encode `value` as a canonical-JSON length-prefixed frame and write it.
///
/// Flushes after the write to guarantee the kernel has the whole frame
/// before the matching `read_frame` is issued.
pub async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), BusClientError>
where
    W: AsyncWriteExt + Unpin + Send + ?Sized,
    T: Serialize + Sync + ?Sized,
{
    let frame = famp_bus::codec::encode_frame(value).map_err(BusClientError::Frame)?;
    writer.write_all(&frame).await.map_err(BusClientError::Io)?;
    writer.flush().await.map_err(BusClientError::Io)?;
    Ok(())
}

/// Read exactly one canonical-JSON frame from `reader` and decode it.
///
/// Reads the 4-byte BE length prefix first, validates it against
/// `MAX_FRAME_BYTES` and the BUS-06 zero-length rule, then reads the
/// payload in one `read_exact` and strict-parses it via `famp_canonical`.
pub async fn read_frame<R, T>(reader: &mut R) -> Result<T, BusClientError>
where
    R: AsyncReadExt + Unpin + Send + ?Sized,
    T: DeserializeOwned,
{
    let mut len_buf = [0u8; famp_bus::codec::LEN_PREFIX_BYTES];
    reader
        .read_exact(&mut len_buf)
        .await
        .map_err(BusClientError::Io)?;
    let len = u32::from_be_bytes(len_buf);
    if len == 0 {
        return Err(BusClientError::Frame(
            famp_bus::codec::FrameError::EmptyFrame,
        ));
    }
    if (len as usize) > famp_bus::codec::MAX_FRAME_BYTES {
        return Err(BusClientError::Frame(
            famp_bus::codec::FrameError::FrameTooLarge(len),
        ));
    }
    let mut body = vec![0u8; len as usize];
    reader
        .read_exact(&mut body)
        .await
        .map_err(BusClientError::Io)?;
    let value: T = famp_canonical::from_slice_strict(&body).map_err(BusClientError::Decode)?;
    Ok(value)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use famp_bus::{BusMessage, BusReply};
    use tokio::io::duplex;

    #[tokio::test]
    async fn write_then_read_round_trips_busmessage() {
        let (mut a, mut b) = duplex(1024);
        let original = BusMessage::Hello {
            bus_proto: 1,
            client: "famp-cli/0.9.0".into(),
        };
        write_frame(&mut a, &original).await.unwrap();
        let decoded: BusMessage = read_frame(&mut b).await.unwrap();
        assert_eq!(original, decoded);
    }

    #[tokio::test]
    async fn write_then_read_round_trips_busreply() {
        let (mut a, mut b) = duplex(1024);
        let original = BusReply::HelloOk { bus_proto: 1 };
        write_frame(&mut a, &original).await.unwrap();
        let decoded: BusReply = read_frame(&mut b).await.unwrap();
        assert_eq!(original, decoded);
    }
}
