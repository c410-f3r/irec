//! Initial endpoints

use crate::{
  api::{FILE_EXTENSION, MAX_FRAMES},
  irec_dir, FileTy,
};
use std::{fmt::Write, time::SystemTime};
use tokio::{
  fs::{create_dir_all, OpenOptions},
  io::{AsyncWriteExt, BufWriter},
  net::TcpStream,
};
use tokio_rustls::server::TlsStream;
use wtx::{
  rng::StdRng,
  web_socket::{FrameBufferVec, OpCode, WebSocketServer},
  PartitionedBuffer,
};

/// Stores an arbitrary sequence of bytes into a ".webm" file located at the home directory.
#[inline]
pub async fn store(
  fb: &mut FrameBufferVec,
  ft: FileTy,
  ws: &mut WebSocketServer<(), PartitionedBuffer, StdRng, &mut TlsStream<TcpStream>>,
) -> crate::Result<()> {
  let mut irec_dir = irec_dir(ft)?;
  if !irec_dir.exists() {
    create_dir_all(&irec_dir).await?;
  }
  let file = OpenOptions::new()
    .create(true)
    .write(true)
    .open({
      irec_dir.push(&file_name(ft)?);
      let _ = irec_dir.set_extension(FILE_EXTENSION);
      irec_dir
    })
    .await?;
  let mut bw = BufWriter::new(file);
  for _ in 0..MAX_FRAMES {
    let frame = ws.read_frame(fb).await?;
    match frame.op_code() {
      OpCode::Binary => {
        bw.write_all(frame.fb().payload()).await?;
      }
      OpCode::Close => {
        bw.flush().await?;
        return Ok(());
      }
      OpCode::Continuation | OpCode::Text | OpCode::Ping | OpCode::Pong => {
        return Err(crate::Error::UnexpectedFrameOpCode);
      }
    }
  }
  Err(crate::Error::LargeAmountOfFrames)
}

fn file_name(ft: FileTy) -> crate::Result<String> {
  let now = SystemTime::now();
  let timestamp = now.duration_since(std::time::UNIX_EPOCH)?.as_nanos();
  let mut timestamp_string = String::with_capacity(16);
  timestamp_string.write_fmt(format_args!("irec-{}-{timestamp}", <&str>::from(ft)))?;
  Ok(timestamp_string)
}
