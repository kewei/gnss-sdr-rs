use crate::acquisition::do_acquisition::{AcquisitionResult, ChannelState};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::error::Error;
use std::sync::PoisonError;

#[derive(Debug, Clone)]
pub struct TrackingError;
impl std::fmt::Display for TrackingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrackingError")
    }
}

impl Error for TrackingError {}

impl<T> From<PoisonError<T>> for TrackingError {
    fn from(_: PoisonError<T>) -> Self {
        TrackingError
    }
}

pub struct TrackingChannel {
    pub prn: u8,
    pub state: ChannelState,

    pub carrier_freq: f32,
    pub code_phase: f32,
    pub code_rate: f32,

    pub i_prompt: f32,
    pub q_prompt: f32,

    pub pll_filter: PllSecondOrderFilter,
    pub dll_filter: DllSecondOrderFilter,
}

impl TrackingChannel {
    pub fn new(prn: u8) -> Self{
        Self {
            prn,
            state: ChannelState::Idle,
        }
    }

    pub fn start(&mut self, result: AcquisitionResult) {
        self.state = ChannelState::Tracking(result.prn);
    }
}

pub struct PllSecondOrderFilter {
    // PLL filter state and parameters
}

pub struct DllSecondOrderFilter {
    // DLL filter state and parameters
}

pub struct TrackingManager {
    pub channels: Vec<TrackingChannel>,
    pub active_prns: Arc<RwLock<HashSet<u8>>>,
}

impl TrackingManager {
    pub fn new(num_chnls: usize) -> Self {
        Self {
            channels: (0..num_chnls).map(|prn| TrackingChannel::new(prn as u8)).collect(),
            active_prns: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub fn assign_tracking(&mut self, result: AcquisitionResult) -> Result<(), TrackingError> {
        if let Some(channel) = self.channels.iter_mut().find(|c| c.state == ChannelState::Idle) {
            self.active_prns.write()?.insert(result.prn);
            channel.start(result);
        }
        Ok(())
    }

    pub fn release_tracking(&mut self, prn: u8) -> Result<(), TrackingError> {
        self.channels.iter_mut().find(|c| c.prn== prn).map(|c| c.state = ChannelState::Idle);
        self.active_prns.write()?.remove(&prn);
        Ok(())
    }
}