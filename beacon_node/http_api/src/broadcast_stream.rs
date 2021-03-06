// TODO: this should be replaced with the tokio's `BroadcastStream` once it's added to
// tokio-stream (https://github.com/tokio-rs/tokio/pull/3384)

use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio_stream::Stream;
use tokio_util::sync::ReusableBoxFuture;

/// A wrapper around [`tokio::sync::broadcast::Receiver`] that implements [`Stream`].
///
/// [`tokio::sync::broadcast::Receiver`]: struct@tokio::sync::broadcast::Receiver
/// [`Stream`]: trait@crate::Stream
pub struct BroadcastStream<T> {
    inner: ReusableBoxFuture<(Result<T, RecvError>, Receiver<T>)>,
}

/// An error returned from the inner stream of a [`BroadcastStream`].
#[derive(Debug, PartialEq)]
pub enum BroadcastStreamRecvError {
    /// The receiver lagged too far behind. Attempting to receive again will
    /// return the oldest message still retained by the channel.
    ///
    /// Includes the number of skipped messages.
    Lagged(u64),
}

async fn make_future<T: Clone>(mut rx: Receiver<T>) -> (Result<T, RecvError>, Receiver<T>) {
    let result = rx.recv().await;
    (result, rx)
}

impl<T: 'static + Clone + Send> BroadcastStream<T> {
    /// Create a new `BroadcastStream`.
    pub fn new(rx: Receiver<T>) -> Self {
        Self {
            inner: ReusableBoxFuture::new(make_future(rx)),
        }
    }
}

impl<T: 'static + Clone + Send> Stream for BroadcastStream<T> {
    type Item = Result<T, BroadcastStreamRecvError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (result, rx) = match self.inner.poll(cx) {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        };
        self.inner.set(make_future(rx));
        match result {
            Ok(item) => Poll::Ready(Some(Ok(item))),
            Err(RecvError::Closed) => Poll::Ready(None),
            Err(RecvError::Lagged(n)) => {
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(n))))
            }
        }
    }
}

impl<T> fmt::Debug for BroadcastStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BroadcastStream").finish()
    }
}
