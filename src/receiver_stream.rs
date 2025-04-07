use std::{pin::Pin, task::Poll};

use futures::Stream;
use tokio::sync::broadcast::{Receiver, error::RecvError};

pub struct ReceiverStream<T: Clone + Send + 'static> {
    recv_future: Pin<Box<dyn Future<Output = (Receiver<T>, Result<T, RecvError>)> + Send + Sync>>,
}

impl<T: Clone + Send + 'static> ReceiverStream<T> {
    pub fn new(mut receiver: Receiver<T>) -> Self {
        let recv_future = Box::pin(async {
            let result = receiver.recv().await;
            (receiver, result)
        });
        Self { recv_future }
    }
}

impl<T: Clone + Send + 'static> Stream for ReceiverStream<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.recv_future.as_mut().poll(cx) {
            Poll::Ready((mut receiver, result)) => {
                self.recv_future = Box::pin(async {
                    let result = receiver.recv().await;
                    (receiver, result)
                });
                match result {
                    Ok(value) => Poll::Ready(Some(value)),
                    Err(RecvError::Lagged(_)) => Poll::Pending,
                    Err(RecvError::Closed) => Poll::Ready(None),
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
