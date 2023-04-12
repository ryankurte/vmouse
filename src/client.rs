
use std::os::unix::prelude::AsRawFd;
use std::pin::Pin;
use std::task::Poll;

use async_std::os::unix::net::UnixStream;
use futures::{AsyncRead, AsyncWriteExt, Stream};
use log::{trace, debug};

use crate::Command;


#[derive(Clone, Debug)]
pub struct Client {
    path: String,
    stream: UnixStream,
}

impl Client {
    pub async fn connect(path: String) -> Result<Self, std::io::Error> {
        // Connect to daemon socket
        let stream = UnixStream::connect(&path).await?;

        Ok(Self { path, stream })
    }

    pub async fn send(&mut self, cmd: Command) -> Result<(), anyhow::Error> {
        let encoded: Vec<u8> = bincode::serialize(&cmd)?;

        debug!("Send: {:?}", cmd);

        let _n = self.stream.write_all(&encoded).await?;

        Ok(())
    }
}

impl Stream for Client {
    type Item = Result<Command, anyhow::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut buff = vec![0u8; 1024];

        let n = match Pin::new(&mut self.stream).poll_read(cx, &mut buff) {
            Poll::Ready(Ok(n)) => n,
            Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e.into()))),
            Poll::Pending => return Poll::Pending,
        };

        let decoded: Command = match bincode::deserialize(&buff[..n]) {
            Ok(d) => d,
            Err(e) => return Poll::Ready(Some(Err(e.into()))),
        };

        trace!("Receive: {:?}", decoded);

        Poll::Ready(Some(Ok(decoded)))
    }
}

impl std::hash::Hash for Client {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.stream.as_raw_fd().hash(state);
    }
}
