use config::Config;
use crate::transforms::{DQCurrents, DQVoltages};
use remote_obj::*;
use bincode::{Encode, Decode};

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode, Debug))]
pub struct PController {
    pub k_p: f32,
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
#[remote(derive(Encode, Decode, Debug))]
pub struct PIController {
    pub k_i: f32,
    pub i_error: f32,
    pub p_controller: PController,
}

impl PIController {
    pub fn new(k_i: f32, k_p: f32) -> PIController {
        PIController {
            k_i,
            i_error: 0.0,
            p_controller: PController::new(k_p),
        }
    }

    pub fn update(&mut self, error: f32, saturated: bool) -> f32 {
        if !saturated {
            self.i_error += error;
        }
        self.p_controller.update(error) + self.k_i * self.i_error
    }
}

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode, Debug))]
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

    pub fn update(&mut self, current_inputs: &DQCurrents, current_requests: &DQCurrents, config: &Config) -> DQVoltages {
        self.d_controller.k_i = config.current_controller_k_i;
        self.q_controller.k_i = config.current_controller_k_i;
        self.d_controller.p_controller.k_p = config.current_controller_k_p;
        self.q_controller.p_controller.k_p = config.current_controller_k_p;

        DQVoltages {
            d: -self.d_controller.update(-(current_inputs.d - current_requests.d), false),
            q: -self.q_controller.update(-(current_inputs.q - current_requests.q), false),
        }
    }
}