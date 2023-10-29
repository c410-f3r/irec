use irec::{api::v1::store, FileTy};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::{env, fs, io, str, sync::Arc};
use tokio::{io::AsyncWriteExt, net::TcpListener};
use tokio_rustls::{
  rustls::{Certificate, PrivateKey, ServerConfig},
  TlsAcceptor,
};
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
  let uri_parts_from_req = UriParts::from(str::from_utf8(req.path()).unwrap_or_default());
  let question_fn = || {
    let mut iter = uri_parts_from_req.href.split('?');
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
  let tls_acceptor = tls_acceptor(cert, key)?;
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
          key_buffer: &mut <_>::default(),
          pb: PartitionedBuffer::default(),
          rng: StdRng::default(),
          stream: &mut tls_stream,
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

fn tls_acceptor(mut cert: &[u8], mut key: &[u8]) -> wtx::Result<TlsAcceptor> {
  let key = pkcs8_private_keys(&mut key)?
    .into_iter()
    .map(PrivateKey)
    .next()
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No private key"))?;
  let certs: Vec<_> = certs(&mut cert)?.into_iter().map(Certificate).collect();
  let config = ServerConfig::builder()
    .with_safe_defaults()
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
  Ok(TlsAcceptor::from(Arc::new(config)))
}
enum Endpoint {
  Store,
}

#[cfg(test)]
mod tests {
  use crate::serve;
  use irec::{irec_dir, FileTy};
  use std::{io::Cursor, sync::Arc, time::Duration};
  use tokio::{
    fs::{read_dir, read_to_string, remove_dir_all},
    net::TcpStream,
    time::sleep,
  };
  use tokio_rustls::{
    rustls::{ClientConfig, OwnedTrustAnchor, RootCertStore, ServerName},
    TlsConnector,
  };
  use webpki_roots::TLS_SERVER_ROOTS;
  use wtx::{
    rng::StdRng,
    web_socket::{
      handshake::{WebSocketConnect, WebSocketConnectRaw},
      FrameBufferVec, FrameMutVec, OpCode,
    },
    PartitionedBuffer,
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
      pb: PartitionedBuffer::default(),
      rng: StdRng::default(),
      stream: tls_connector()
        .connect(
          ServerName::try_from("localhost").map_err(|_err| wtx::Error::MissingHost).unwrap(),
          TcpStream::connect("localhost:3000").await.unwrap(),
        )
        .await
        .unwrap(),
      uri: &format!("ws://localhost:3000/store?ft=audio,key={key}"),
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

  fn tls_connector() -> TlsConnector {
    let mut root_store = RootCertStore::empty();
    root_store.add_trust_anchors(TLS_SERVER_ROOTS.iter().map(|ta| {
      OwnedTrustAnchor::from_subject_spki_name_constraints(ta.subject, ta.spki, ta.name_constraints)
    }));
    let _ = root_store.add_parsable_certificates(
      &rustls_pemfile::certs(&mut Cursor::new(include_bytes!("../../../.certs/root-ca.crt")))
        .unwrap(),
    );
    let config = ClientConfig::builder()
      .with_safe_defaults()
      .with_root_certificates(root_store)
      .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
  }
}
