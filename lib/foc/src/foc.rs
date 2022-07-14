use crate::calibration::EncoderCalibration;
use crate::config::Config;
use crate::pid::DQCurrentController;
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};
use crate::transforms::{DQCurrents, DQVoltages};

pub struct FieldOrientedControl {
    cal: EncoderCalibration,
    current_controller: DQCurrentController,
}

impl FieldOrientedControl {
    pub fn new(cal: EncoderCalibration, config: &Config) -> FieldOrientedControl {
        FieldOrientedControl {
            cal,
            current_controller: DQCurrentController::new(config),
        }
    }

    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        let pos = update.position.unwrap();
        let angle = self.cal.to_angle(pos, config);

        let dq_currents = update.phase_currents
            .clarke_transform()
            .park_transform(angle);

        // PI controllers goes here, voltage as output and current error as input

        let _voltage_request = DQVoltages {
             d: 0.0,
             q: config.open_loop_voltage
        };

        let voltage_request = self.current_controller.update(
            &dq_currents,
            &DQCurrents{
                d: 0.0,
                q: 1.0
            });

        voltage_request
            .inv_park_transform(angle)
            .to_voltage_controller_output()
    }
}