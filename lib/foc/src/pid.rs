use crate::config::Config;
use crate::transforms::{DQCurrents, DQVoltages};
use remote_obj::*;
use bincode::{Encode, Decode};

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode))]
pub struct PController {
    k_p: f32,
}

impl PController {
    pub fn new(k_p: f32) -> PController {
        PController { k_p }
    }

    pub fn update(&self, error: f32) -> f32 {
        self.k_p * error
    }
}

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode))]
pub struct PIController {
    k_i: f32,
    i_error: f32,
    p_controller: PController,
}

impl PIController {
    pub fn new(k_i: f32, k_p: f32) -> PIController {
        PIController {
            k_i,
            i_error: 0.0,
            p_controller: PController::new(k_p),
        }
    }

    pub fn update(&mut self, error: f32) -> f32 {
        self.i_error += error;
        self.p_controller.update(error) + self.k_i * self.i_error
    }
}

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode))]
pub struct DQCurrentController {
    d_controller: PIController,
    q_controller: PIController,
}

impl DQCurrentController {
    pub fn new(config: &Config) -> DQCurrentController {
        let k_p = config.current_controller_k_p;
        let k_i = config.current_controller_k_i;

        DQCurrentController {
            d_controller: PIController::new(k_i, k_p),
            q_controller: PIController::new(k_i, k_p),
        }
    }

    pub fn update(&mut self, current_inputs: &DQCurrents, current_requests: &DQCurrents) -> DQVoltages {
        DQVoltages {
            d: -self.d_controller.update(current_inputs.d - current_requests.d),
            q: -self.q_controller.update(current_inputs.q - current_requests.q),
        }
    }
}