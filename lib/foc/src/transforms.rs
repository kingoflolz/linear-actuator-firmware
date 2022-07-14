use libm::sincosf;
use crate::state_machine::VoltageControllerOutput;

pub struct AlphaBetaCurrents {
    pub alpha: f32, // units of amps
    pub beta: f32,
}

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
}

pub struct DQCurrents {
    pub d: f32, // units of amps
    pub q: f32,
}

impl AlphaBetaCurrents {
    pub fn park_transform(&self, angle: f32) -> DQCurrents {
        let (s, c) = sincosf(angle);

        DQCurrents {
            d: self.alpha * c + self.beta * s,
            q: self.beta * c - self.alpha * s,
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
    pub fn to_voltage_controller_output(&self) -> VoltageControllerOutput {
        VoltageControllerOutput {
            driver_enable: true,
            alpha: self.alpha,
            beta: self.beta,
        }
    }
}

impl DQVoltages {
    fn inv_park_transform(&self, angle: f32) -> AlphaBetaVoltages {
        let (s, c) = sincosf(angle);

        AlphaBetaVoltages {
            alpha: self.d * c - self.q * s,
            beta: self.q * c + self.d * s,
        }
    }
}