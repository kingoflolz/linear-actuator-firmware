use libm::sincosf;
use crate::config::Config;
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};
use remote_obj::*;
use bincode::{Encode, Decode};

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode))]
pub struct OpenLoopVoltageController {
    pub(crate) position: f32
}

impl OpenLoopVoltageController {
    pub fn new() -> OpenLoopVoltageController {
        OpenLoopVoltageController {
            position: 0.0
        }
    }

    // get position in mm
    pub fn get_position(&self, config: &Config) -> f32 {
        self.position / core::f32::consts::TAU * config.motor_len_per_cycle
    }

    // units of velocity_req is electrical radians per controller timestep
    pub fn process_velocity(&mut self, velocity_req: f32, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        let output = self.process_position(self.position, &update, &config);
        self.position += velocity_req;

        output
    }

    pub fn process_position(&mut self, position_req: f32, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        let voltage = update.bus_voltage;
        let request_duty = config.open_loop_voltage / voltage;

        let (s, c) = sincosf(position_req);

        let alpha = s * request_duty;
        let beta = c * request_duty;

        self.position = position_req;

        VoltageControllerOutput {
            driver_enable: true,
            alpha,
            beta
        }
    }
}