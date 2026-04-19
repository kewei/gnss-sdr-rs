use crate::acquisition::acquisition::{AcquisitionResult, ChannelState};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub struct TrackingChannel {
    state: ChannelState,
}

pub struct TrackingManager {
    channels: Vec<TrackingChannel>,
    active_prns: Arc<Mutex<HashSet<u8>>>,
}

impl TrackingManager {
    pub fn new(count: usize) -> Self {
        Self {
            channels: (0..count).map(|_| TrackingChannel::new()).collect(),
            active_prns: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn assign_channel(&mut self, result: AcquisitionResult) {
        if let Some(channel) = self.channels.iter_mut().find(|c| c.state == ChannelState::Idle) {
            channel.start(result);
            self.active_prns.lock()?.insert(result.prn);
        }
    }
}