use tokio::sync::{mpsc, Mutex, MutexGuard};

#[derive(Debug)]
pub struct Mailbox<T>
where
    T: Send + Sync,
{
    sender: mpsc::UnboundedSender<T>,
    receiver: Mutex<mpsc::UnboundedReceiver<T>>,
}

impl<T> Default for Mailbox<T>
where
    T: Send + Sync,
{
    fn default() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            sender,
            receiver: Mutex::new(receiver),
        }
    }
}

impl<'mailbox, T> Mailbox<T>
where
    T: Send + Sync,
{
    pub fn get_sender(&'mailbox self) -> &'mailbox mpsc::UnboundedSender<T> {
        &self.sender
    }

    pub async fn lock_receiver(&'mailbox self) -> MutexGuard<'mailbox, mpsc::UnboundedReceiver<T>> {
        self.receiver.lock().await
    }
}
