//! Server

#![allow(clippy::panic, clippy::std_instead_of_alloc)]

use irec::{api::v1::store, FileTy};
use std::{env, fs, str};
use tokio::{io::AsyncWriteExt, net::TcpListener};
use wtx::{
  http::Request,
  misc::{TokioRustlsAcceptor, Uri},
  rng::StdRng,
  web_socket::{
    handshake::{WebSocketAccept, WebSocketAcceptRaw},
    FrameBufferVec, WebSocketBuffer,
  },
};

#[tokio::main]
async fn main() -> irec::Result<()> {
  let mut args = env::args();
  let (_, Some(cert_file), Some(key_file)) = (args.next(), args.next(), args.next()) else {
    panic!("Missing certificate and key");
  };
  let cert = fs::read(cert_file)?;
  let key = fs::read(key_file)?;
  serve("0.0.0.0:3000", &cert, &key, |key_from_req| {
    let Ok(key_from_env) = env::var("IREC_KEY") else {
      return false;
    };
    key_from_env == key_from_req
  })
  .await
}

fn manage_accept(
  endpoint: &mut Endpoint,
  ft: &mut FileTy,
  req: &dyn Request,
  key_cb: &mut impl FnMut(&str) -> bool,
) -> bool {
  let uri_parts_from_req = Uri::new(str::from_utf8(req.path()).unwrap_or_default());
  let question_fn = || {
    let mut iter = uri_parts_from_req.href().split('?');
    let before_question = iter.next()?;
    let after_question = iter.next().unwrap_or("");
    Some((before_question, after_question))
  };
  let (before_question, after_question) = question_fn().unwrap_or_default();
  let mut has_ft = false;
  let mut has_key = false;

  *endpoint = match before_question {
    "/store" => Endpoint::Store,
    _ => return false,
  };

  for by_comma in after_question.split(',').take(2) {
    if let Some(ft_str) = by_comma.split("ft=").nth(1) {
      if let Ok(local_ft) = FileTy::try_from(ft_str) {
        has_ft = true;
        *ft = local_ft;
      }
    } else if let Some(key_from_req) = by_comma.split("key=").nth(1) {
      if key_cb(key_from_req) {
        has_key = true;
      }
    } else {
    }
  }

  has_ft && has_key
}

async fn serve(
  addr: &str,
  cert: &[u8],
  key: &[u8],
  mut key_cb: impl Copy + FnMut(&str) -> bool + Send + 'static,
) -> irec::Result<()> {
  let listener = TcpListener::bind(addr).await?;
  let tls_acceptor = TokioRustlsAcceptor::default().with_cert_chain_and_priv_key(cert, key)?;
  loop {
    let (stream, _) = listener.accept().await?;
    let local_tls_acceptor = tls_acceptor.clone();
    let _jh = tokio::spawn(async move {
      let fun = || async move {
        let mut endpoint = Endpoint::Store;
        let mut ft = FileTy::Audio;
        let mut tls_stream = local_tls_acceptor.accept(stream).await?;
        let ws_rslt = WebSocketAcceptRaw {
          compression: (),
          rng: StdRng::default(),
          stream: &mut tls_stream,
          wsb: WebSocketBuffer::default(),
        }
        .accept(|req| manage_accept(&mut endpoint, &mut ft, req, &mut key_cb))
        .await;
        let mut ws = match ws_rslt {
          Err(err) => {
            tls_stream.write_all(b"HTTP/1.1 500\r\n").await?;
            return Err(err.into());
          }
          Ok(elem) => elem,
        };
        let mut fb = FrameBufferVec::default();
        match endpoint {
          Endpoint::Store => store(&mut fb, ft, &mut ws).await?,
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
  Store,
}

#[cfg(test)]
mod tests {
  use crate::serve;
  use irec::{irec_dir, FileTy};
  use std::{net::ToSocketAddrs, time::Duration};
  use tokio::{
    fs::{read_dir, read_to_string, remove_dir_all},
    time::sleep,
  };
  use wtx::{
    misc::{TokioRustlsConnector, Uri},
    rng::StdRng,
    web_socket::{
      handshake::{WebSocketConnect, WebSocketConnectRaw},
      FrameBufferVec, FrameMutVec, OpCode, WebSocketBuffer,
    },
  };

  #[tokio::test]
  async fn store() {
    let irec_dir = irec_dir(FileTy::Audio).unwrap();
    if irec_dir.exists() {
      remove_dir_all(&irec_dir).await.unwrap();
    }

    let content = "123";
    let key = "abc";

    let _server = tokio::spawn(serve(
      "localhost:3000",
      include_bytes!("../../../.certs/cert.pem"),
      include_bytes!("../../../.certs/key.pem"),
      move |key_from_req| key_from_req == key,
    ));
    sleep(Duration::from_millis(500)).await;
    let mut fb = FrameBufferVec::default();
    let (_, mut ws) = WebSocketConnectRaw {
      compression: (),
      fb: &mut fb,
      headers_buffer: &mut <_>::default(),
      rng: StdRng::default(),
      stream: TokioRustlsConnector::from_webpki_roots()
        .push_certs(include_bytes!("../../../.certs/root-ca.crt"))
        .unwrap()
        .with_tcp_stream("localhost:3000".to_socket_addrs().unwrap().next().unwrap(), "localhost")
        .await
        .unwrap(),
      uri: &Uri::new(&format!("ws://localhost:3000/store?ft=audio,key={key}")),
      wsb: WebSocketBuffer::default(),
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
