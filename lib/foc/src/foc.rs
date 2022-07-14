use crate::calibration::EncoderCalibration;
use crate::config::Config;
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};
use crate::transforms::DQVoltages;

pub struct FieldOrientedControl {
    cal: EncoderCalibration,
}

impl FieldOrientedControl {
    pub fn new(cal: EncoderCalibration) -> FieldOrientedControl {
        FieldOrientedControl {
            cal
        }
    }

    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        let pos = update.position.unwrap();
        let angle = self.cal.to_angle(pos, config);

        let _dq_currents = update.phase_currents
            .clarke_transform()
            .park_transform(angle);

        // PI controllers goes here, voltage as output and current error as input

        let voltage_request = DQVoltages {
            d: 0.0,
            q: 1.5
        };

        voltage_request.inv_park_transform(angle).to_voltage_controller_output()
    }
}