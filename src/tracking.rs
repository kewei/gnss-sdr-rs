use std::error::Error;

use crate::acquisition::AcquistionStatistics;

pub struct TrackingStatistics {}

pub fn do_track(acq_res: AcquistionStatistics) -> Result<TrackingStatistics, Box<dyn Error>> {
    todo!();
}
