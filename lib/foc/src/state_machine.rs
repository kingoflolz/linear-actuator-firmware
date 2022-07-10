use crate::config::Config;
use crate::open_loop_velocity::OpenLoopVelocityController;
use crate::svm::IterativeSVM;

pub struct LowLevelControllerOutput {
    pub driver_enable: bool,
    pub alpha: f32,
    pub beta: f32,
}

pub struct Controller {
    svm: IterativeSVM,
    open_loop_velocity: OpenLoopVelocityController,
}

impl Controller {
    pub fn new(config: &Config) -> Controller {
        let cycle_time = config.switching_clock_frequency / config.switching_frequency;
        let dead_time_cycles = config.cycle_deadtime * config.switching_clock_frequency;

        Controller {
            svm: IterativeSVM::new(dead_time_cycles as u16,
                                   cycle_time as u16),
            open_loop_velocity: OpenLoopVelocityController::new(),
        }
    }

    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> PWMCommand {
        let open_loop_velocity_output = self.open_loop_velocity.process(0.01, update, config);
        self.svm.calculate(open_loop_velocity_output)
    }
}

pub struct PWMCommand {
    pub driver_enable: bool,
    pub u_duty: u16,
    pub v_duty: u16,
    pub w_duty: u16,
}

impl PWMCommand {
    pub fn to_array(&self) -> [u16; 3] {
        [self.u_duty, self.v_duty, self.w_duty]
    }
}

pub struct ControllerUpdate {
    pub u_current: f32,
    pub v_current: f32,
    pub w_current: f32,

    pub bus_voltage: f32,

    pub position: Option<f32>,
}