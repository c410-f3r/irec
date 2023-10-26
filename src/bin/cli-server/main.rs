use std::str;

use irec::{api::v1::store_file, FileTy};
use tokio::{io::AsyncWriteExt, net::TcpListener};
use wtx::{
  http::Request,
  rng::StdRng,
  web_socket::{
    handshake::{WebSocketAccept, WebSocketAcceptRaw},
    FrameBufferVec,
  },
  PartitionedBuffer, UriParts,
};

#[tokio::main]
async fn main() -> irec::Result<()> {
  serve("0.0.0.0:3000", |key_from_req| {
    let Ok(key_from_env) = std::env::var("IREC_KEY") else {
      return false;
    };
    key_from_env == key_from_req
  })
  .await
}

fn manage_accept(
  endpoint: &mut Endpoint,
  ft: &mut Option<FileTy>,
  req: &dyn Request,
  cb: fn(&str) -> bool,
) -> bool {
  let uri_parts_from_req = UriParts::from(str::from_utf8(req.path()).unwrap_or_default());
  let question_fn = || {
    let mut iter = uri_parts_from_req.href.split('?');
    let before_question = iter.next()?;
    let after_question = iter.next().unwrap_or("");
    Some((before_question, after_question))
  };
  let (before_question, after_question) = question_fn().unwrap_or_default();
  let mut has_key = false;

  *endpoint = match before_question {
    "/store-file" => Endpoint::StoreFile,
    "/stream-audio" => Endpoint::StreamAudio,
    "/stream-video" => Endpoint::StreamVideo,
    _ => return false,
  };

  for by_comma in after_question.split(',').take(2) {
    if let Some(ft_str) = by_comma.split("ft=").nth(1) {
      if let Ok(local_ft) = FileTy::try_from(ft_str) {
        *ft = Some(local_ft);
      }
    } else if let Some(key_from_req) = by_comma.split("key=").nth(1) {
      if cb(key_from_req) {
        has_key = true;
      }
    }
  }

  has_key
}

async fn serve(addr: &str, cb: fn(&str) -> bool) -> irec::Result<()> {
  let listener = TcpListener::bind(addr).await?;
  loop {
    let (mut stream, _) = listener.accept().await?;
    let _jh = tokio::spawn(async move {
      let fun = || async move {
        let mut endpoint = Endpoint::StoreFile;
        let mut ft = None;
        let ws_rslt = WebSocketAcceptRaw {
          compression: (),
          key_buffer: &mut <_>::default(),
          pb: PartitionedBuffer::default(),
          rng: StdRng::default(),
          stream: &mut stream,
        }
        .accept(|req| manage_accept(&mut endpoint, &mut ft, req, cb))
        .await;
        let mut ws = match ws_rslt {
          Err(err) => {
            stream.write_all(b"HTTP/1.1 500\r\n").await?;
            return Err(err.into());
          }
          Ok(elem) => elem,
        };
        let mut fb = FrameBufferVec::default();
        match endpoint {
          Endpoint::StoreFile => store_file(&mut fb, ft.unwrap(), &mut ws).await?,
          Endpoint::StreamAudio => {}
          Endpoint::StreamVideo => {}
        }
        irec::Result::Ok(())
      };

      if let Err(err) = fun().await {
        panic!("{err}");
      }
    });
  }
}

enum Endpoint {
  StoreFile,
  StreamAudio,
  StreamVideo,
}

#[cfg(test)]
mod tests {
  use crate::serve;
  use irec::irec_dir;
  use std::time::Duration;
  use tokio::{
    fs::{read_dir, read_to_string, remove_dir_all},
    net::TcpStream,
    time::sleep,
  };
  use wtx::{
    rng::StdRng,
    web_socket::{
      handshake::{WebSocketConnect, WebSocketConnectRaw},
      FrameBufferVec, FrameMutVec, OpCode,
    },
    PartitionedBuffer,
  };

  #[tokio::test]
  async fn store_file() {
    let irec_dir = irec_dir().unwrap();
    remove_dir_all(&irec_dir).await.unwrap();

    const KEY: &str = "abc";
    let content = "123";

    let _server = tokio::spawn(serve("0.0.0.0:3000", |key_from_req| key_from_req == KEY));
    sleep(Duration::from_millis(500)).await;

    let mut fb = FrameBufferVec::default();
    let (_, mut ws) = WebSocketConnectRaw {
      compression: (),
      fb: &mut fb,
      headers_buffer: &mut <_>::default(),
      pb: PartitionedBuffer::default(),
      rng: StdRng::default(),
      stream: TcpStream::connect("0.0.0.0:3000").await.unwrap(),
      uri: &format!("ws://0.0.0.0:3000/store-file?ft=audio,key={KEY}"),
    }
    .connect()
    .await
    .unwrap();
    ws.write_frame(&mut FrameMutVec::new_fin(&mut fb, OpCode::Binary, content.as_bytes()).unwrap())
      .await
      .unwrap();
    ws.write_frame(&mut FrameMutVec::new_fin(&mut fb, OpCode::Close, &[]).unwrap()).await.unwrap();
    sleep(Duration::from_millis(500)).await;

    let mut read_dir = read_dir(irec_dir).await.unwrap();
    let dir_entry = read_dir.next_entry().await.unwrap().unwrap();
    assert!(read_dir.next_entry().await.unwrap().is_none());
    assert_eq!(read_to_string(dir_entry.path()).await.unwrap(), content)
  }
}
