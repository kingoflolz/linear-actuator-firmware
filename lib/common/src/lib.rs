#![no_std]
use bincode::{Decode, Encode};
use foc::state_machine::ControllerUpdate;
use foc::config::Config;
use foc::transforms::{DQCurrents, PhaseCurrents};
use encoder::EncoderOutput;

#[derive(Encode, Decode, Debug, Clone, PartialEq)]
pub struct Sample {
    pub id: u16,
    pub adc: [u16; 16],
    pub pwm: [u16; 3],
    pub dq_currents: Option<DQCurrents>,
}

pub fn to_controller_update(adc_buf: &[u16; 16], position: &Option<EncoderOutput>, config: &Config) -> ControllerUpdate {
    fn adc_to_voltage(adc: u16) -> f32 {
        adc as f32 / 4096.0 * 3.3
    }

    let vbus_s_pin = adc_to_voltage(adc_buf[13]);

    // 10k:1k voltage divider
    let vbus = vbus_s_pin * 11.0;

    // 5mv/A
    fn adc_to_current(adc: i16) -> f32 {
        adc as f32 / 4096.0 * 3.3 / 0.005
    }

    ControllerUpdate {
        phase_currents: PhaseCurrents{
            u: adc_to_current(adc_buf[10] as i16 - adc_buf[9] as i16),
            v: adc_to_current(adc_buf[11] as i16 - adc_buf[9] as i16),
            w: adc_to_current(adc_buf[12] as i16 - adc_buf[9] as i16),
        }.normalize(),
        bus_voltage: vbus,
        position: position.clone().map(|position|
            position.position / core::f32::consts::TAU * config.encoder_len_per_cycle
        ),
    }
}