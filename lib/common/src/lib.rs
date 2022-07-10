#![no_std]
use bincode::{Decode, Encode};
use foc::state_machine::ControllerUpdate;

#[derive(Encode, Decode, Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub id: u16,
    pub adc: [u16; 10],
    pub pwm: [u16; 3]
}

pub fn adc_buf_to_controller_update(adc_buf: &[u16; 10]) -> ControllerUpdate {
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
        u_current: adc_to_current(adc_buf[5]),
        v_current: adc_to_current(adc_buf[6]),
        w_current: adc_to_current(adc_buf[7]),
        bus_voltage: vbus,
        position: None
    }
}