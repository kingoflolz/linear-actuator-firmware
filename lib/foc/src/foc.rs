use crate::calibration::EncoderCalibration;
use config::Config;
use crate::pid::{DQCurrentController, PController, PIController};
use crate::state_machine::{ControllerUpdate, VoltageControllerOutput};
use crate::transforms::DQCurrents;
use remote_obj::*;
use bincode::{Encode, Decode};
use encoder::EncoderOutput;

#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode, Debug))]
pub struct PosController {
    pub vel_controller: PIController,
    pub pos_controller: PController,

    pub pos_setpoint: f32,
    pub vel_setpoint: f32,
}

impl PosController {
    fn update(&mut self, encoder: &EncoderOutput, config: &Config) -> f32 {
        self.vel_controller.k_i = config.vel_controller_k_i;
        self.vel_controller.p_controller.k_p = config.vel_controller_k_p;
        self.pos_controller.k_p = config.pos_controller_k_p;

        let velocity_setpoint = self.pos_controller.update(self.pos_setpoint - encoder.filtered_position);
        self.vel_setpoint = velocity_setpoint;
        return self.vel_controller.update(encoder.velocity - self.vel_setpoint);
    }
}


#[derive(Debug, RemoteGetter, RemoteSetter)]
#[remote(derive(Encode, Decode, Debug))]
pub struct FieldOrientedControl {
    cal: EncoderCalibration,
    current_controller: DQCurrentController,
    dq_currents: DQCurrents,
    q_req: f32,
    pos_controller: PosController,
    encoder_output: EncoderOutput,
}

impl FieldOrientedControl {
    pub fn new(cal: EncoderCalibration, config: &Config) -> FieldOrientedControl {
        FieldOrientedControl {
            cal,
            current_controller: DQCurrentController::new(config),
            dq_currents: DQCurrents::default(),
            q_req: 0.0,
            pos_controller: PosController {
                vel_controller: PIController::new(config.vel_controller_k_i, config.vel_controller_k_p),
                pos_controller: PController::new(config.pos_controller_k_p),
                pos_setpoint: -50.0,
                vel_setpoint: 0.0
            },
            encoder_output: EncoderOutput::default(),
        }
    }

    pub fn update(&mut self, update: &ControllerUpdate, config: &Config) -> VoltageControllerOutput {
        // encoder output is in terms of mm
        let encoder_output = update.position.as_ref().unwrap();
        self.encoder_output = encoder_output.clone();
        let angle = self.cal.to_angle(self.encoder_output.position, config);

        let dq_currents = update.phase_currents
            .clarke_transform()
            .park_transform(angle);

        // let voltage_request = DQVoltages {
        //      d: 0.0,
        //      q: config.open_loop_voltage
        // };

        let q = self.pos_controller.update(encoder_output, config);
        let q = q.max(-config.curr_limit).min(config.curr_limit);

        let voltage_request = self.current_controller.update(
            &dq_currents,
            &DQCurrents{
                d: 0.0,
                q,
            },
            &config);

        self.dq_currents = dq_currents;
        self.q_req = q;

        voltage_request
            .inv_park_transform(angle)
            .to_voltage_controller_output(update)
    }
}