use crate::config::Config;
use crate::svm::IterativeSVM;
use crate::calibration::EncoderCalibrationController;
use crate::foc::FieldOrientedControl;
use crate::transforms::{DQCurrents, PhaseCurrents};
use remote_obj::*;
use bincode::{Encode, Decode};

pub struct VoltageControllerOutput {
    pub driver_enable: bool,
    pub alpha: f32, // units of duty cycle
    pub beta: f32,
}

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode, Debug))]
pub enum VoltageController {
    Cal(EncoderCalibrationController),
    Foc(FieldOrientedControl),
}

impl VoltageController {
    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        match self {
            VoltageController::Cal(cal) => {
                if cal.is_done() {
                    self.enter_foc(config)
                }
            },
            _ => {}
        }

        match self {
            VoltageController::Cal(cal) => {
                cal.update(update, config)
            }
            VoltageController::Foc(foc) => {
                foc.update(update, config)
            }
        }
    }

    pub fn enter_foc(&mut self, config: &Config) {
        match self {
            VoltageController::Cal(cal) => {
                let foc = FieldOrientedControl::new(cal.get_calib().unwrap(), config);
                *self = VoltageController::Foc(foc);
            }
            _ => {}
        }
    }
}

#[derive(RemoteGetter, RemoteSetter, Debug)]
#[remote(derive(Encode, Decode, Debug))]
pub struct Controller {
    svm: IterativeSVM,
    voltage_controller: VoltageController,
}

impl Controller {
    pub fn new(config: &Config) -> Controller {
        let cycle_time = config.switching_clock_frequency / config.switching_frequency;
        let dead_time_cycles = config.cycle_deadtime * config.switching_clock_frequency;

        Controller {
            svm: IterativeSVM::new(dead_time_cycles as u16,
                                   cycle_time as u16),
            voltage_controller: VoltageController::Cal(EncoderCalibrationController::new()),
        }
    }

    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> PWMCommand {
        let voltage_output = self.voltage_controller.update( update, config);

        let mut command = self.svm.calculate(voltage_output);
        if update.bus_voltage < config.uvlo {
            command.driver_enable = false;
        }
        command
    }

    pub fn encoder_ready(&self) -> bool {
        match &self.voltage_controller {
            VoltageController::Cal(c) => {
                c.encoder_ready()
            }
            VoltageController::Foc(_) => {
                true
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(RemoteGetter, RemoteSetter, Debug, Clone, PartialEq)]
#[remote(derive(Encode, Decode, Debug))]
pub struct ControllerUpdate {
    pub phase_currents: PhaseCurrents,
    pub bus_voltage: f32,
    #[remote(skip)]
    pub position: Option<f32>,
}