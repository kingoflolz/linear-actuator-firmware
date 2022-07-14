use crate::calibration::EncoderCalibration;
use crate::config::Config;
use crate::open_loop_voltage::OpenLoopVoltageController;
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};

pub struct FieldOrientedControl {
    cal: EncoderCalibration,
    open_loop: OpenLoopVoltageController
}

impl FieldOrientedControl {
    pub fn new(cal: EncoderCalibration) -> FieldOrientedControl {
        FieldOrientedControl {
            cal,
            open_loop: OpenLoopVoltageController::new()
        }
    }

    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        let pos = update.position.unwrap();
        let angle = self.cal.to_angle(pos, config) + core::f32::consts::FRAC_PI_2;

        self.open_loop.process_position(angle, update, config)
    }
}