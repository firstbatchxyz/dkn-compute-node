///! https://docs.rs/libp2p-gossipsub/latest/libp2p_gossipsub/trait.DataTransform.html
use libp2p::gossipsub::{DataTransform, Message, RawMessage, TopicHash};
use std::io::{Error, ErrorKind};
use std::time::{SystemTime, UNIX_EPOCH};

/// A `DataTransform` implementation that adds & checks a timestamp to the message.
pub struct TTLDataTransform {
    /// Time-to-live, e.g. obtained from some `duration.as_secs()`.
    ttl_secs: u64,
}

impl TTLDataTransform {
    const MID_SIZE: usize = 8;

    #[inline(always)]
    fn get_time_secs(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

impl DataTransform for TTLDataTransform {
    fn inbound_transform(&self, mut raw_message: RawMessage) -> Result<Message, Error> {
        // check source
        if raw_message.source.is_none() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Message source is None",
            ));
        }

        // check length (panic condition for `split_off` as well)
        if Self::MID_SIZE > raw_message.data.len() {
            return Err(Error::new(ErrorKind::InvalidInput, "Message too short"));
        }

        // parse time
        let raw_data = raw_message.data.split_off(Self::MID_SIZE);
        let msg_time = u64::from_be_bytes(raw_message.data[0..Self::MID_SIZE].try_into().unwrap());

        // check ttl
        if msg_time + self.ttl_secs < self.get_time_secs() {
            return Err(Error::new(ErrorKind::InvalidInput, "Message TTL expired"));
        }

        Ok(Message {
            source: raw_message.source,
            data: raw_data,
            sequence_number: raw_message.sequence_number,
            topic: raw_message.topic,
        })
    }

    fn outbound_transform(
        &self,
        _topic: &TopicHash,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, std::io::Error> {
        let msg_time = self.get_time_secs().to_be_bytes();

        // prepend time bytes to the data
        let mut transformed_data = Vec::with_capacity(Self::MID_SIZE + data.len());
        transformed_data.extend_from_slice(&msg_time);
        transformed_data.extend_from_slice(&data);

        Ok(transformed_data)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use libp2p::PeerId;

    use super::*;

    #[test]
    fn test_ttl_data_transform() {
        let data = vec![1, 2, 3, 4, 5];
        let ttl_secs = Duration::from_secs(100).as_secs();
        let ttl_data_transform = TTLDataTransform { ttl_secs };
        let topic = TopicHash::from_raw("topic");

        // outbound transform
        let transformed_data = ttl_data_transform
            .outbound_transform(&topic, data.clone())
            .unwrap();

        // inbound transform
        let raw_message = RawMessage {
            source: Some(PeerId::random()),
            data: transformed_data,
            sequence_number: None,
            topic,
            signature: Default::default(),
            key: Default::default(),
            validated: false,
        };
        let message = ttl_data_transform.inbound_transform(raw_message).unwrap();

        assert_eq!(message.data, data);
    }
}
