use crate::acquisition::do_acquisition::{AcquisitionResult, ChannelState};
use crate::utilities::multicast_ring_buffer::MulticastRingBuffer;
use crate::utilities::ca_code::generate_ca_code_samples;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::error::Error;
use std::sync::PoisonError;
use crossbeam_channel::{Sender, Receiver};
use num::complex::Complex;

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

pub struct TrackingMessage {
    SatelliteLost(u8),
    SatelliteLocked(u8),
}

pub struct TrackingChannel {
    pub id: u8,
    pub prn: u8,
    pub state: ChannelState,
    pub next_sample_index: usize,

    pub fs: f32,
    pub carrier_freq: f32,
    pub code_phase: f32,
    pub code_rate: f32,

    pub i_prompt: f32,
    pub q_prompt: f32,

    pub pll_filter: PllSecondOrderFilter,
    pub dll_filter: DllSecondOrderFilter,
}

impl TrackingChannel {
    pub fn new(id: u8) -> Self{
        Self {
            id,
            prn: 0,
            state: ChannelState::Idle,
            next_sample_index: 0,
            fs: 0.0,
            carrier_freq: 0.0,
            code_phase: 0.0,
            code_rate: 0.0,
            i_prompt: 0.0,
            q_prompt: 0.0,
            pll_filter: PllSecondOrderFilter{},
            dll_filter: DllSecondOrderFilter{},
        }
    }

    pub fn start(&mut self, result: AcquisitionResult) {
        self.prn = result.prn;
        self.carrier_freq = result.carrier_freq;
        self.code_phase = result.code_phase as f32;
        self.code_rate = 0.0;
        self.fs = result.fs;
        self.state = ChannelState::Tracking(result.prn);
    }

    pub fn is_active(&self) -> bool {
        self.state == ChannelState::Tracking(self.prn)
    }

    pub fn update(&mut self, buff: MulticastRingBuffer) {
        if self.state != ChannelState::Tracking(self.prn) {
            return;
        }

        let samples_ca_code = generate_ca_code_samples(self.prn, self.fs);
        let samples_per_code = samples_ca_code.len();

        let head = buff.get_head();

        if head < self.next_sample_index + samples_per_code {
            return;
        }

        let mut samples = vec![Complex::<f32>::new(0.0, 0.0); samples_per_code];

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
    pub acq_to_trk: Receiver<AcquisitionResult>,
    pub trk_to_acq: Sender<TrackingMessage>,
}

impl TrackingManager {
    pub fn new(num_chnls: usize, acq_to_trk: Receiver<AcquisitionResult>, trk_to_acq: Sender<TrackingMessage>) -> Self {
        Self {
            channels: (1..num_chnls + 1).map(|prn| TrackingChannel::new(prn as u8 )).collect(),
            acq_to_trk,
            trk_to_acq,
        }
    }

    pub fn run(&mut self, multi_ring_buf: Arc<MulticastRingBuffer>) {
        while let Ok(msg) = self.acq_to_trk.try_recv() {
            if let Some(channel) = self.channels.iter_mut().find(|c| c.state == ChannelState::Idle) {
                channel.start(msg);
                let _ = self.trk_to_acq.send(TrackingMessage::SatelliteLocked(msg.prn));
            }
        }

        for channel in self.channels.iter_mut().filter(|c| c.is_active()) {
            channel.update(multi_ring_buf.clone());

            if channel.state == ChannelState::Lost {
                let _ = self.trk_to_acq.send(TrackingMessage::SatelliteLost(channel.prn));
                channel.reset();
            }
        }
    }
}