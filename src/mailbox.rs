use tokio::sync::broadcast;

#[derive(Debug)]
pub struct Mailbox<T, const DEFAULT_CAPACITY: usize> {
    pub broadcast: broadcast::Sender<T>,
}

impl<T, const DEFAULT_CAPACITY: usize> Default for Mailbox<T, DEFAULT_CAPACITY> {
    fn default() -> Self {
        Self {
            broadcast: broadcast::Sender::new(DEFAULT_CAPACITY),
        }
    }
}
