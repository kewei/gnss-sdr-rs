use crate::acquisition::{do_acquisition, AcquisitionResult};
use crate::app_buffer_utilities::{APPBUFF, BUFFER_SIZE};
use crate::decoding::{nav_decoding, NavSyncStatus};
use crate::gps_constants;
use crate::tracking::{do_track, TrackingResult};
use crate::view::{self, NavigationView};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::time::Duration;

const RETRY_INTERVAL: u64 = 10; // Seconds

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessStage {
    SignalAcquisition,
    SignalTracking,
    //MessageDecoding,
}

pub fn do_data_process(
    prn: usize,
    freq_sampling: f32,
    freq_IF: f32,
    stage_thread: Arc<Mutex<ProcessStage>>,
    acquisition_result_thread: Arc<Mutex<AcquisitionResult>>,
    tracking_result_thread: Arc<Mutex<TrackingResult>>,
    nav_stat_thread: Arc<Mutex<NavSyncStatus>>,
    is_complex: bool,
    cnt_each: Arc<Mutex<usize>>,
    sender_thread: Sender<NavigationView>,
    term_signal: Arc<AtomicBool>,
) {
    let mut buffer_location = 0;
    let mut cnt = cnt_each.lock().expect("Error in locking cnt");

    while term_signal.load(Ordering::SeqCst) {
        let stage_thread_clone = Arc::clone(&stage_thread);
        let mut stage = stage_thread_clone
            .lock()
            .expect("Error in locking 'ProcessStage' in thread");
        let mut nav_view = NavigationView::new(prn);

        match *stage {
            ProcessStage::SignalAcquisition => {
                let acq_result_clone = acquisition_result_thread.clone();
                if let Ok(buf_location) =
                    do_acquisition(acq_result_clone, freq_sampling, freq_IF, is_complex)
                {
                    buffer_location = buf_location;
                    let acq_result_clone2 = acquisition_result_thread.clone();
                    let acq_result = acq_result_clone2
                        .lock()
                        .expect("Error in locking after acquisition");

                    println!(
                        "prn: {} freq: {} code_phase: {}",
                        acq_result.prn, acq_result.carrier_freq, acq_result.code_phase
                    );
                    nav_view.prn = acq_result.prn;
                    nav_view.acq_mag = acq_result.mag_relative;

                    *stage = ProcessStage::SignalTracking;
                }
                /*else {
                    let acq_result_clone2 = acquisition_result_thread.clone();
                    println!(
                        "PRN {} is not present, retry in {}s",
                        acq_result_clone2
                            .try_lock()
                            .expect("Error in locking AcquisitionResult after acquisition")
                            .prn,
                        RETRY_INTERVAL
                    );
                    thread::sleep(Duration::from_secs(RETRY_INTERVAL));
                };
                */
            }

            ProcessStage::SignalTracking => {
                let mut code_freq: f32 = 0.0;
                {
                    let trk_result_clone = tracking_result_thread.clone();
                    let trk_result = trk_result_clone
                        .lock()
                        .expect("Error in locking TrackingResult");
                    code_freq = trk_result.code_freq;
                }
                let code_phase_step: f32 = code_freq / freq_sampling;
                let num_ca_code_samples =
                    (gps_constants::GPS_L1_CA_CODE_LENGTH_CHIPS / code_phase_step).ceil() as usize;
                let app_buff_clone = unsafe { Arc::clone(&APPBUFF) };
                let app_buff_value = app_buff_clone
                    .read()
                    .expect("Error in reading buff_cnt in acquisition");
                let buffer_location_curr =
                    app_buff_value.buff_cnt * BUFFER_SIZE - num_ca_code_samples;

                if buffer_location_curr > buffer_location {
                    if let Ok(buffer_loc) = do_track(
                        acquisition_result_thread.clone(),
                        tracking_result_thread.clone(),
                        freq_sampling,
                        freq_IF,
                        buffer_location,
                    ) {
                        let trk_result_clone = tracking_result_thread.clone();
                        let mut trk_result = trk_result_clone
                            .lock()
                            .expect("Error in locking 'TrackingResult' thread");
                        buffer_location = buffer_loc;
                        trk_result.buff_location = buffer_loc as u64;
                        if nav_view.trk_I_P.len() == view::LENGTH_VIEW_DATA {
                            nav_view.trk_I_P.pop_front();
                            nav_view.trk_Q_P.pop_front();
                        }
                        nav_view.trk_I_P.push_back(trk_result.i_prompt);
                        nav_view.trk_Q_P.push_back(trk_result.q_prompt);
                        if *cnt % view::LENGTH_VIEW_DATA == 0 {
                            sender_thread.send(nav_view).unwrap();
                        }
                    } else {
                        println!("Tracking failed.");
                    };
                    *stage = ProcessStage::SignalTracking;
                    *cnt += 1;
                    nav_decoding(
                        tracking_result_thread.clone(),
                        buffer_location,
                        *cnt,
                        nav_stat_thread.clone(),
                    );
                } else {
                    //sleep(Duration::from_millis(1)).await;
                    thread::sleep(Duration::from_millis(1));
                }
            } /* ProcessStage::MessageDecoding => {
                  let trk_result_clone = tracking_result_thread.clone();
                  if let Ok(pos_result) = nav_decoding(trk_result_clone, *cnt, &mut nav_sync_status) {
                      *stage = ProcessStage::SignalTracking;
                  } else {
                      todo!(); // do tracking again with new data
                  }
              } */
        }
    }
}

mod test {
    use super::*;
    use crate::acquisition::{do_acquisition, PRN_SEARCH_ACQUISITION_TOTAL};
    use crate::test_utilities::plot_samples;
    use crate::test_utilities::read_data_file;
    use binrw::BinReaderExt;
    use crossbeam_channel::unbounded;
    use std::fs::File;
    use std::io::Read;
    use std::thread;
    use std::time::Duration;
    use std::time::Instant;
    use tokio::runtime::Runtime;

    #[test]
    fn test_data_process() {
        use tokio::task;
        let t1: Instant = Instant::now();
        let f_name = "/home/kewei/Downloads/rtlsdr_l1/rtlsdr_l1.bin"; //"src/test_data/GPS_recordings/gioveAandB_short.bin";
        let f_sampling: f32 = 2.048e6;
        let f_inter_freq: f32 = 0e6;

        let (m_sender, m_receiver) = unbounded::<NavigationView>();

        // Ctrl-C interruption
        let term = Arc::new(AtomicBool::new(true));
        let term_r = term.clone();

        ctrlc::set_handler(move || {
            term_r.store(false, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
        let mut handlers = Vec::new();

        handlers.push(
            thread::Builder::new()
                .name("Device reader".to_string())
                .spawn(move || {
                    read_data_file(f_name);
                })
                .unwrap(),
        );

        thread::sleep(Duration::from_millis(500));

        let mut acquisition_results: Vec<Arc<Mutex<AcquisitionResult>>> = Vec::new();
        let mut tracking_results: Vec<Arc<Mutex<TrackingResult>>> = Vec::new();
        let mut stages_all: Vec<Arc<Mutex<ProcessStage>>> = Vec::new();
        let mut nav_stats_all: Vec<Arc<Mutex<NavSyncStatus>>> = Vec::new();
        for i in 1..=PRN_SEARCH_ACQUISITION_TOTAL {
            let acq_result: AcquisitionResult = AcquisitionResult::new(i, f_sampling);
            acquisition_results.push(Arc::new(Mutex::new(acq_result)));
            let trk_result = TrackingResult::new(i);
            tracking_results.push(Arc::new(Mutex::new(trk_result)));
            let nav_stat = NavSyncStatus::new();
            nav_stats_all.push(Arc::new(Mutex::new(nav_stat)));
            stages_all.push(Arc::new(Mutex::new(ProcessStage::SignalAcquisition)));
            let nav_view = NavigationView::new(i);
        }
        let mut cnt_all: Vec<Arc<Mutex<usize>>> = Vec::with_capacity(PRN_SEARCH_ACQUISITION_TOTAL);
        (0..PRN_SEARCH_ACQUISITION_TOTAL).for_each(|_| cnt_all.push(Arc::new(Mutex::new(0))));

        for i in 0..PRN_SEARCH_ACQUISITION_TOTAL {
            let acq_result_clone = Arc::clone(&acquisition_results[i]);
            let trk_result_clone = Arc::clone(&tracking_results[i]);
            let nav_stat_clone = Arc::clone(&nav_stats_all[i]);
            let stage_clone = Arc::clone(&stages_all[i]);
            let stop_signal_clone = Arc::clone(&term);
            let cnt_each = Arc::clone(&cnt_all[i]);
            let sender_clone = m_sender.clone();
            handlers.push(
                thread::Builder::new()
                    .name(format!("PRN: {i}").to_string())
                    .spawn(move || {
                        do_data_process(
                            i,
                            f_sampling,
                            f_inter_freq,
                            stage_clone,
                            acq_result_clone,
                            trk_result_clone,
                            nav_stat_clone,
                            false,
                            cnt_each,
                            sender_clone,
                            stop_signal_clone,
                        );
                    })
                    .unwrap(),
            );
        }

        for handle in handlers {
            handle.join().unwrap();
            thread::sleep(Duration::from_millis(200));
        }
    }
}
