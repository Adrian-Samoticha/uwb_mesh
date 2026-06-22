use std::collections::VecDeque;
use std::sync::Mutex;

/// A message queue implementation for the HardwareInterface.
///
/// This message queue stores messages as a queue of vectors of u8. The
/// queue is implemented using a VecDeque, which provides a FIFO ordering
/// guarantee and thread-safe access through a Mutex.
pub struct MessageQueue {
    queue: Mutex<VecDeque<Vec<u8>>>,
    total_size_in_bytes: Mutex<usize>,
    max_size_in_bytes: usize,
    size_prefix_size_in_bytes: usize,
}

impl MessageQueue {
    const SIZE_PREFIX_SIZE_IN_BYTES: usize = 4;

    /// Creates a new MessageQueue with no size limit.
    pub fn new() -> Self {
        MessageQueue {
            queue: Mutex::new(VecDeque::new()),
            total_size_in_bytes: Mutex::new(0),
            max_size_in_bytes: usize::MAX,
            size_prefix_size_in_bytes: Self::SIZE_PREFIX_SIZE_IN_BYTES,
        }
    }

    /// Creates a new MessageQueue with a maximum size limit.
    pub fn with_max_size(max_size_in_bytes: usize) -> Self {
        MessageQueue {
            queue: Mutex::new(VecDeque::new()),
            total_size_in_bytes: Mutex::new(0),
            max_size_in_bytes,
            size_prefix_size_in_bytes: Self::SIZE_PREFIX_SIZE_IN_BYTES,
        }
    }

    /// Push a message onto the queue. If the message would exceed the
    /// maximum size, it will not be pushed onto the queue.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to push onto the queue.
    ///
    /// # Returns
    ///
    /// `true` if the message was pushed onto the queue, `false` otherwise.
    pub fn push(&self, message: Vec<u8>) -> bool {
        let mut total_size = self.total_size_in_bytes.lock().unwrap();
        let message_size = message.len() + self.size_prefix_size_in_bytes;

        if *total_size + message_size > self.max_size_in_bytes {
            return false;
        }

        let mut queue = self.queue.lock().unwrap();
        queue.push_back(message);
        *total_size += message_size;

        true
    }

    /// Pop a message from the queue.
    ///
    /// # Returns
    ///
    /// `Some(message)` if the queue is not empty, `None` otherwise.
    pub fn pop(&self) -> Option<Vec<u8>> {
        let mut queue = self.queue.lock().unwrap();

        if let Some(message) = queue.pop_front() {
            let mut total_size = self.total_size_in_bytes.lock().unwrap();
            *total_size -= message.len() + self.size_prefix_size_in_bytes;
            Some(message)
        } else {
            None
        }
    }

    /// Check if the queue is empty.
    ///
    /// # Returns
    ///
    /// `true` if the queue is empty, `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }

    /// Get the total size in bytes of all messages in the queue.
    ///
    /// # Returns
    ///
    /// The total size in bytes of all messages in the queue.
    pub fn total_size_in_bytes(&self) -> usize {
        *self.total_size_in_bytes.lock().unwrap()
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}
