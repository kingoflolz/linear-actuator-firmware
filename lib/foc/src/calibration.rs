use crate::state_machine::{ControllerUpdate, ControllerOutput};
use crate::config::Config;

pub enum EncoderCalibrationStatus {
    Start,
    Move1 {
        step: u32
    },
    Move2 {
        step: u32
    },
    End,
}

impl EncoderCalibrationStatus {
    pub fn next(&mut self, update: &ControllerUpdate, config: &Config) {
        match self {
            EncoderCalibrationStatus::Start => {}
            EncoderCalibrationStatus::Move1 { .. } => {}
            EncoderCalibrationStatus::Move2 { .. } => {}
            EncoderCalibrationStatus::End => {}
        }

        let voltage = update.bus_voltage;
        let request_duty = config.calibration_voltage / voltage;

        let sqrt3_by_2 = 0.86602540378f32;
    }
}