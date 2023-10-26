//! Initial endpoints

use crate::{
  api::{CONTAINER_EXTENSION, MAX_FRAMES},
  irec_dir, FileTy,
};
use std::{fmt::Write, time::SystemTime};
use tokio::{
  fs::{create_dir_all, OpenOptions},
  io::{AsyncWriteExt, BufWriter},
  net::TcpStream,
};
use wtx::{
  rng::StdRng,
  web_socket::{FrameBufferVec, OpCode, WebSocketServer},
  PartitionedBuffer,
};

/// Stores an arbitrary sequence of bytes into a ".webm" file located at the home directory.
#[inline]
pub async fn store_file(
  fb: &mut FrameBufferVec,
  ft: FileTy,
  ws: &mut WebSocketServer<(), PartitionedBuffer, StdRng, &mut TcpStream>,
) -> crate::Result<()> {
  let irec_dir = irec_dir()?;
  if !irec_dir.exists() {
    create_dir_all(&irec_dir).await?;
  }
  let oo = OpenOptions::new()
    .create(true)
    .write(true)
    .open(irec_dir.join(&file_name(ft)?).with_extension(CONTAINER_EXTENSION))
    .await?;
  let mut bw = BufWriter::new(oo);
  for _ in 0..MAX_FRAMES {
    let frame = ws.read_frame(fb).await?;
    match frame.op_code() {
      OpCode::Binary => {}
      OpCode::Close => {
        bw.flush().await?;
        return Ok(());
      }
      OpCode::Continuation | OpCode::Text | OpCode::Ping | OpCode::Pong => {
        return Err(crate::Error::UnexpectedFrameOpCode);
      }
    }
    bw.write_all(frame.fb().payload()).await?;
  }
  Err(crate::Error::LargeAmountOfFrames)
}

/// TODO
#[cfg(feature = "wry")]
pub async fn stream_audio(
  _: &mut FrameBufferVec,
  _: &mut WebSocketServer<(), PartitionedBuffer, StdRng, &mut TcpStream>,
) -> crate::Result<()> {
  Ok(())
}

/// TODO
#[cfg(feature = "wry")]
pub async fn stream_video(
  _: &mut FrameBufferVec,
  _: &mut WebSocketServer<(), PartitionedBuffer, StdRng, &mut TcpStream>,
) -> crate::Result<()> {
  Ok(())
}

fn file_name(ft: FileTy) -> crate::Result<String> {
  let now = SystemTime::now();
  let timestamp = now.duration_since(std::time::UNIX_EPOCH)?.as_nanos();
  let mut timestamp_string = String::with_capacity(16);
  timestamp_string.write_fmt(format_args!("irec-{}-{timestamp}", <&str>::from(ft)))?;
  Ok(timestamp_string)
}
