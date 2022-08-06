// use rtt_target::rprintln;
use config::Config;
use crate::open_loop_voltage::OpenLoopVoltageController;
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};
use encoder::normalizer::{NormalizerBuilder, Normalizer};
use remote_obj::*;
use bincode::{Encode, Decode};

#[derive(Debug, Clone, Eq, PartialEq, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode, Debug))]
pub enum EncoderCalibrationState {
    Start (u32),
    ToEndstop,
    Calib1,
    Calib2,
    Done1,
    Done2,
}

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode, Debug))]
pub struct EncoderCalibrationController {
    pub state: EncoderCalibrationState,
    position_target: f32, // in units of electrical radians
    pub open_loop: OpenLoopVoltageController,
    calib1_builder: NormalizerBuilder,
    calib2_builder: NormalizerBuilder,
}

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode, Debug))]
pub struct EncoderCalibration {
    offset: f32
}

impl EncoderCalibration {
    pub fn to_angle(&self, encoder_value: f32, config: &Config) -> f32 {
        (encoder_value - self.offset) / config.motor_len_per_cycle * core::f32::consts::TAU
    }
}

impl EncoderCalibrationController {
    pub fn new() -> EncoderCalibrationController {
        EncoderCalibrationController {
            state: EncoderCalibrationState::Start (0),
            position_target: 0.0,
            open_loop: OpenLoopVoltageController::new(),
            calib1_builder: NormalizerBuilder::new(),
            calib2_builder: NormalizerBuilder::new(),
        }
    }

    pub fn encoder_ready(&self) -> bool {
        match self.state {
            EncoderCalibrationState::Calib1 |
            EncoderCalibrationState::Calib2 |
            EncoderCalibrationState::Done1 |
            EncoderCalibrationState::Done2 => {
                true
            }
            _ => {
                false
            }
        }
    }

    pub fn is_done(&self) -> bool {
        match self.state {
            EncoderCalibrationState::Done1 |
            EncoderCalibrationState::Done2 => {
                true
            }
            _ => {
                false
            }
        }
    }

    pub fn get_calib_raw(&self) -> Option<[Normalizer; 2]> {
        let l = self.calib1_builder.get_normalizer();
        let r = self.calib2_builder.get_normalizer();

        match (l, r) {
            (Some(l), Some(r)) => Some([l, r]),
            _ => None
        }
    }

    pub fn get_calib(&self) -> Option<EncoderCalibration> {
        let [l, r] = self.get_calib_raw()?;
        let offset = (l.mean + r.mean) / 2.0;
        Some(EncoderCalibration {offset})
    }

    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        let dir;
        let cal_length = config.calibration_length / config.motor_len_per_cycle // calibration length in motor cycles
                            * core::f32::consts::TAU;

        // rprintln!("{}, {:?}", self.open_loop_velocity.position, self.state);

        match &mut self.state {
            EncoderCalibrationState::Start(wait) => {
                *wait += 1;
                if *wait > 1_000 {
                    self.state = EncoderCalibrationState::ToEndstop;
                    self.position_target = cal_length;
                }
                dir = 0.0;
            }
            EncoderCalibrationState::ToEndstop => {
                if self.open_loop.position > self.position_target {
                    self.state = EncoderCalibrationState::Calib1;
                    self.position_target = 0.0;
                }
                dir = 1.0;
            }
            state @ EncoderCalibrationState::Calib1 |
            state @ EncoderCalibrationState::Calib2 => {
                // get open loop position request and encoder position in units of mm
                let position = update.position.as_ref().unwrap().position;
                let position_target = self.open_loop.get_position(&config);

                let error = position - position_target;
                let norm_builder;
                match state {
                     EncoderCalibrationState::Calib1 => {
                         if self.open_loop.position < self.position_target {
                             self.state = EncoderCalibrationState::Calib2;
                             self.position_target = cal_length;
                         }
                         dir = -1.0;
                         norm_builder = &mut self.calib1_builder;
                     }
                    EncoderCalibrationState::Calib2 => {
                        if self.open_loop.position > self.position_target {
                            self.state = EncoderCalibrationState::Done1;
                            self.position_target = 0.0;
                        }
                        dir = 1.0;
                        norm_builder = &mut self.calib2_builder;
                    }
                    _ => {unreachable!()}
                }
                norm_builder.update(error);
            }
            EncoderCalibrationState::Done1 => {
                if self.open_loop.position < self.position_target {
                    self.state = EncoderCalibrationState::Done2;
                    self.position_target = cal_length;
                }
                dir = -1.0;
            }
            EncoderCalibrationState::Done2 => {
                if self.open_loop.position > self.position_target {
                    self.state = EncoderCalibrationState::Done1;
                    self.position_target = 0.0;
                }
                dir = 1.0;
            }
        }

        let mut v_out = self.open_loop.process_velocity(0.0005 * dir, update, config);
        if dir == 0.0 {
            v_out.driver_enable = false;
        }
        v_out
    }
}