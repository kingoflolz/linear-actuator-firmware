use libm::{cosf, sinf};
use crate::config::Config;
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};

pub struct OpenLoopVelocityController {
    pub(crate) position: f32
}

impl OpenLoopVelocityController {
    pub fn new() -> OpenLoopVelocityController {
        OpenLoopVelocityController{
            position: 0.0
        }
    }

    // units of velocity_req is electrical radians per controller timestep
    pub fn process(&mut self, velocity_req: f32, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        let voltage = update.bus_voltage;
        let request_duty = config.calibration_voltage / voltage;

        let alpha = sinf(self.position) * request_duty;
        let beta = cosf(self.position) * request_duty;

        self.position += velocity_req;

        VoltageControllerOutput {
            driver_enable: true,
            alpha,
            beta
        }
    }
}