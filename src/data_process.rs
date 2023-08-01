use crate::acquisition::PRN_SEARCH_ACQUISITION_TOTAL;
use crate::acquisition::{do_acquisition, AcquisitionResult};
use crate::app_buffer_utilities::get_current_buffer;
use crate::decoding::nav_decoding;
use crate::gps_constants;
use crate::tracking::{do_track, TrackingResult};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::task;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessStage {
    SignalAcquisition,
    SignalTracking,
    MessageDecoding,
}

pub async fn do_data_process(
    freq_sampling: f32,
    freq_IF: f32,
    stage_thread: Arc<Mutex<ProcessStage>>,
    acquisition_result_thread: Arc<Mutex<AcquisitionResult>>,
    tracking_result_thread: Arc<Mutex<TrackingResult>>,
    is_complex: bool,
    buffer_location: usize,
    term_signal: Arc<AtomicBool>,
) {
    while !term_signal.load(Ordering::SeqCst) {
        let mut stage = stage_thread
            .lock()
            .expect("Error in locking 'ProcessStage' in thread");

        match *stage {
            ProcessStage::SignalAcquisition => {
                let acq_result_clone = acquisition_result_thread.clone();
                if let Ok(()) = do_acquisition(
                    acq_result_clone,
                    freq_sampling,
                    freq_IF,
                    buffer_location,
                    is_complex,
                ) {
                    let acq_result_clone2 = acquisition_result_thread.clone();
                    let acq_result = acq_result_clone2
                        .lock()
                        .expect("Error in locking after acquisition");
                    let trk_result_clone = tracking_result_thread.clone();
                    let mut trk_result = trk_result_clone
                        .lock()
                        .expect("Error in locking TrackingResult in acquisition");
                    trk_result.carrier_freq = acq_result.carrier_freq;

                    let mut ca_code = acq_result.ca_code.clone();
                    ca_code.insert(
                        0,
                        ca_code[gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS as usize - 1],
                    );
                    ca_code.push(ca_code[1]);

                    let code_phase_step: f32 =
                        gps_constants::GPS_L1_CA_CODE_RATE_CHIPS_PER_S / freq_sampling;
                    let num_ca_code_samples = (gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS
                        / code_phase_step)
                        .ceil() as usize;
                    let ca_code_prompt: Vec<f32> = (0..num_ca_code_samples)
                        .map(|x| ca_code[(x as f32 * code_phase_step).ceil() as usize] as f32)
                        .collect();
                    trk_result.ca_code_prompt = ca_code_prompt;

                    *stage = ProcessStage::SignalTracking;
                } else {
                };
            }

            ProcessStage::SignalTracking => {
                let acq_result_clone = acquisition_result_thread.clone();
                let trk_result_clone = tracking_result_thread.clone();
                if let Ok(()) = do_track(
                    acq_result_clone,
                    trk_result_clone,
                    freq_sampling,
                    freq_IF,
                    buffer_location,
                ) {
                    let trk_result_clone2 = tracking_result_thread.clone();
                    let trk_result = trk_result_clone2
                        .lock()
                        .expect("Error in locking 'TrackingResult' thread");
                    *stage = ProcessStage::SignalTracking;
                } else {
                    todo!(); // do tracking again with new data
                };
            }

            ProcessStage::MessageDecoding => {
                let trk_result_clone = tracking_result_thread.clone();
                if let Ok(pos_result) = nav_decoding(trk_result_clone) {
                } else {
                    todo!(); // do tracking again with new data
                }
            }
        }
    }
}

mod test {
    use super::*;
    use crate::acquisition::do_acquisition;
    use crate::test_utilities::plot_samples;
    use crate::test_utilities::read_data_file;
    use binrw::BinReaderExt;
    use std::fs::File;
    use std::io::Read;
    use std::thread;
    use std::time::Instant;

    #[test]
    fn test_data_process() {
        let t1: Instant = Instant::now();
        let f_name = "src/test_data/GPS_recordings/gioveAandB_short.bin";
        let f_sampling: f32 = 16.3676e6;
        let f_inter_freq: f32 = 4.1304e6;

        // Ctrl-C interruption
        let term = Arc::new(AtomicBool::new(true));
        let r = term.clone();

        let handle = thread::spawn(move || {
            if let Ok(r1) = read_data_file(f_name) {
                print!("Reading reaches the end of the file");
            } else {
                panic!("Error in reading file");
            };
        });
        handle.join().unwrap();

        let mut acquisition_results: Vec<Arc<Mutex<AcquisitionResult>>> = Vec::new();
        let mut tracking_results: Vec<Arc<Mutex<TrackingResult>>> = Vec::new();
        let mut stages_all: Vec<Arc<Mutex<ProcessStage>>> = Vec::new();
        for i in 1..=PRN_SEARCH_ACQUISITION_TOTAL {
            let acq_result: AcquisitionResult = AcquisitionResult::new(i, f_sampling);
            acquisition_results.push(Arc::new(Mutex::new(acq_result)));
            let trk_result = TrackingResult::new(i);
            tracking_results.push(Arc::new(Mutex::new(trk_result)));
            stages_all.push(Arc::new(Mutex::new(ProcessStage::SignalAcquisition)));
        }

        for i in 0..PRN_SEARCH_ACQUISITION_TOTAL {
            let acq_result_clone = Arc::clone(&acquisition_results[i]);
            let trk_result_clone = Arc::clone(&tracking_results[i]);
            let stage_clone = Arc::clone(&stages_all[i]);
            let stop_signal_clone = Arc::clone(&term);
            task::spawn(async move {
                do_data_process(
                    f_sampling,
                    f_inter_freq,
                    stage_clone,
                    acq_result_clone,
                    trk_result_clone,
                    false,
                    0,
                    stop_signal_clone,
                )
                .await;
            });
        }
    }
}
