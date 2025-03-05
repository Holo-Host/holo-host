#![allow(dead_code)]

use async_nats::Message;

pub struct NatsMessage {
    subject: String,
    reply: Option<String>,
    payload: Vec<u8>,
}

impl NatsMessage {
    pub fn new(subject: impl Into<String>, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            subject: subject.into(),
            reply: None,
            payload: payload.into(),
        }
    }

    pub fn into_message(self) -> Message {
        Message {
            subject: self.subject.into(),
            reply: self.reply.map(|r| r.into()),
            payload: self.payload.clone().into(),
            headers: None,
            status: None,
            description: None,
            length: self.payload.len(),
        }
    }
}
