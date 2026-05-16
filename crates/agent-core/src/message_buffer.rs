use crate::types::Message;

/// A bounded message buffer that replaces the legacy ConversationMemory.
/// Maintains a sliding window of recent messages for agent context.
#[derive(Debug, Clone)]
pub struct MessageBuffer {
    messages: Vec<Message>,
    max_messages: usize,
}

impl MessageBuffer {
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        if self.messages.len() > self.max_messages {
            let to_remove = self.messages.len() - self.max_messages;
            self.messages.drain(0..to_remove);
        }
    }

    pub fn recent_messages(&self, n: usize) -> Vec<Message> {
        self.messages.iter().rev().take(n).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    }

    pub fn all_messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn clear_recent(&mut self, n: usize) {
        let start = self.messages.len().saturating_sub(n);
        self.messages.truncate(start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_bounded() {
        let mut buf = MessageBuffer::new(3);
        buf.add_message(Message::new_user("a"));
        buf.add_message(Message::new_user("b"));
        buf.add_message(Message::new_user("c"));
        buf.add_message(Message::new_user("d"));
        assert_eq!(buf.message_count(), 3);
        assert_eq!(buf.recent_messages(3).len(), 3);
    }

    #[test]
    fn test_clear_recent() {
        let mut buf = MessageBuffer::new(10);
        buf.add_message(Message::new_user("keep"));
        buf.add_message(Message::new_user("remove"));
        buf.clear_recent(1);
        assert_eq!(buf.message_count(), 1);
        assert_eq!(buf.recent_messages(1)[0].content, "keep");
    }
}
