use libm::sincosf;
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};
use bincode::{Decode, Encode};
use remote_obj::*;

pub struct AlphaBetaCurrents {
    pub alpha: f32, // units of amps
    pub beta: f32,
}

#[derive(RemoteGetter, RemoteSetter, Debug, Clone, PartialEq)]
#[remote(derive(Encode, Decode, Debug))]
pub struct PhaseCurrents {
    pub u: f32, // units of amps
    pub v: f32,
    pub w: f32,
}

impl PhaseCurrents {
    pub fn clarke_transform(&self) -> AlphaBetaCurrents {
        let one_by_sqrt3 = 0.57735026919;
        AlphaBetaCurrents {
            alpha: self.u,
            beta: one_by_sqrt3 * (self.v - self.w),
        }
    }

    pub fn normalize(&mut self) -> PhaseCurrents {
        PhaseCurrents {
            u: self.u,
            v: self.v,
            w: self.w,
        }
    }

    pub fn sum_currents(&self) -> f32 {
        self.u + self.v + self.w
    }

    fn max(&self) -> f32 {
        self.u.max(self.v).max(self.w)
    }

    fn min(&self) -> f32 {
        self.u.min(self.v).min(self.w)
    }

    pub fn max_magnitude(&self) -> f32 {
        self.max().max(-self.min())
    }
}


#[derive(RemoteGetter, RemoteSetter, Encode, Decode, Debug, Clone, PartialEq, Default)]
#[remote(derive(Encode, Decode, Debug))]
pub struct DQCurrents {
    pub d: f32, // units of amps
    pub q: f32,
}

impl AlphaBetaCurrents {
    pub fn park_transform(&self, angle: f32) -> DQCurrents {
        let (s, c) = sincosf(angle);

        DQCurrents {
            q: self.alpha * c - self.beta * s,
            d: self.beta * c + self.alpha * s,
        }
    }
}

pub struct DQVoltages {
    pub d: f32, // units of volts
    pub q: f32,
}

pub struct AlphaBetaVoltages {
    pub alpha: f32, // units of volts
    pub beta: f32,
}

impl AlphaBetaVoltages {
    pub fn to_voltage_controller_output(&self, update: &ControllerUpdate) -> VoltageControllerOutput {
        VoltageControllerOutput {
            driver_enable: true,
            alpha: self.alpha / 15.0,
            beta: self.beta / 15.0,
        }
    }
}

impl DQVoltages {
    pub fn inv_park_transform(&self, angle: f32) -> AlphaBetaVoltages {
        let (s, c) = sincosf(angle);

        AlphaBetaVoltages {
            alpha: self.q * c + self.d * s,
            beta: self.d * c - self.q * s,
        }
    }
}