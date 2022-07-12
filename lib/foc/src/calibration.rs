use rtt_target::rprintln;
use crate::config::Config;
use crate::open_loop_velocity::OpenLoopVelocityController;
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};


#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EncoderCalibrationState {
    Start,
    ToEndstop,
    Calib1,
    Calib2,
    Done
}

pub struct EncoderCalibrationController {
    pub state: EncoderCalibrationState,
    position_target: f32,
    open_loop_velocity: OpenLoopVelocityController,
}

impl EncoderCalibrationController {
    pub fn new() -> EncoderCalibrationController {
        EncoderCalibrationController {
            state: EncoderCalibrationState::Start,
            position_target: 0.0,
            open_loop_velocity: OpenLoopVelocityController::new(),
        }
    }

    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        let dir;
        let cal_length = config.calibration_length * 2.0 * core::f32::consts::PI;

        // rprintln!("{}, {:?}", self.open_loop_velocity.position, self.state);

        match self.state {
            EncoderCalibrationState::Start => {
                self.state = EncoderCalibrationState::ToEndstop;
                self.position_target = cal_length;
                dir = 0.0;
            }
            EncoderCalibrationState::ToEndstop => {
                if self.open_loop_velocity.position > self.position_target {
                    self.state = EncoderCalibrationState::Calib1;
                    self.position_target = 0.0;
                }
                dir = 1.0;
            }
            EncoderCalibrationState::Calib1 => {
                if self.open_loop_velocity.position < self.position_target {
                    self.state = EncoderCalibrationState::Calib2;
                    self.position_target = cal_length;
                }
                dir = -1.0;
            }
            EncoderCalibrationState::Calib2 => {
                if self.open_loop_velocity.position > self.position_target {
                    self.state = EncoderCalibrationState::Done;
                    self.position_target = 0.0;
                }
                dir = 1.0;
            }
            EncoderCalibrationState::Done => {
                dir = 0.0;
            }
        }

        let mut v_out = self.open_loop_velocity.process(0.001 * dir, update, config);
        if dir == 0.0 {
            v_out.driver_enable = false;
        }
        v_out
    }
}