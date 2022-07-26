#![no_std]
use bincode::{Decode, Encode};
use bincode::de::Decoder;
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use foc::state_machine::ControllerUpdate;
use foc::config::Config;
use foc::transforms::{DQCurrents, PhaseCurrents};
use encoder::EncoderOutput;
use remote_obj::*;
use heapless::Vec;

#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub id: u16,
    pub buf: Vec<u8, 128>
}

impl Encode for Sample {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        Encode::encode(&self.id, encoder)?;
        Encode::encode(&(self.buf.len() as u8), encoder)?;
        for i in self.buf.iter() {
            Encode::encode(i, encoder)?;
        }
        Ok(())
    }
}

impl Decode for Sample {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        Ok(Self {
            id: Decode::decode(decoder)?,
            buf: {
                let len: u8 = Decode::decode(decoder)?;
                let mut x = Vec::new();
                for _ in 0..len {
                    x.push(Decode::decode(decoder)?);
                }
                x
            }
        })
    }
}

#[derive(RemoteGetter)]
#[remote(derive(Encode, Decode))]
pub struct Container<'a> {
    pub adc: &'a mut [u16; 16],
    pub pwm: &'a mut [u16; 3],
    pub controller: &'a mut foc::state_machine::Controller,
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