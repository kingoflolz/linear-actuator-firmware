#![no_std]
use bincode::{Decode, Encode};
use foc::state_machine::ControllerUpdate;
use foc::config::Config;
use foc::transforms::PhaseCurrents;
use encoder::EncoderOutput;
use encoder::normalizer::Normalizer;

#[derive(Encode, Decode, Debug, Clone, PartialEq)]
pub struct Sample {
    pub id: u16,
    pub adc: [u16; 10],
    pub pwm: [u16; 3],
    pub position: Option<f32>,
    pub position_target: f32,
    pub calibration: Option<[Normalizer; 2]>
}

pub fn to_controller_update(adc_buf: &[u16; 10], position: &Option<EncoderOutput>, config: &Config) -> ControllerUpdate {
    fn adc_to_voltage(adc: u16) -> f32 {
        adc as f32 / 4096.0 * 3.3
    }

    let vbus_s_pin = adc_to_voltage(adc_buf[8]);

    // 10k:1k voltage divider
    let vbus = vbus_s_pin * 11.0;

    // 66.6mv/A
    fn adc_to_current(adc: u16) -> f32 {
        (adc as i32 - 2048) as f32 / 4096.0 * 3.3 / 0.0666
    }

    ControllerUpdate {
        phase_currents: PhaseCurrents{
            u: adc_to_current(adc_buf[5]),
            v: adc_to_current(adc_buf[6]),
            w: adc_to_current(adc_buf[7]),
        },
        bus_voltage: vbus,
        position: position.clone().map(|position|
            position.position / core::f32::consts::TAU * config.encoder_len_per_cycle
        ),
    }
}