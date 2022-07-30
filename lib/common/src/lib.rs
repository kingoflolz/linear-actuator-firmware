#![no_std]
use bincode::{Decode, Encode};
use bincode::config::{Configuration, LittleEndian, NoLimit, SkipFixedArrayLength, Fixint, Varint};
use bincode::de::Decoder;
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use foc::state_machine::ControllerUpdate;
use foc::config::Config;
use foc::transforms::{DQCurrents, PhaseCurrents};
use encoder::{EncoderOutput, EncoderState};
use remote_obj::*;
use heapless::Vec;
use bitset_core::BitSet;

pub static BINCODE_CFG: Configuration<LittleEndian, Varint, SkipFixedArrayLength, NoLimit> = bincode::config::standard()
    .with_little_endian()
    .with_variable_int_encoding()
    .skip_fixed_array_length();

#[derive(Debug, Clone, PartialEq)]
pub struct ScopePacket {
    pub id: u32,
    pub probe_valid: u16,
    pub buf: Vec<u8, 128>
}

pub const SCOPE_PROBES: usize = 16;

impl Encode for ScopePacket {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        Encode::encode(&self.id, encoder)?;
        Encode::encode(&self.probe_valid, encoder)?;
        Encode::encode(&(self.buf.len() as u8), encoder)?;
        for i in self.buf.iter() {
            Encode::encode(i, encoder)?;
        }
        Ok(())
    }
}

impl Decode for ScopePacket {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        Ok(Self {
            id: Decode::decode(decoder)?,
            probe_valid: Decode::decode(decoder)?,
            buf: {
                let len: u8 = Decode::decode(decoder)?;
                let mut x = Vec::new();
                for _ in 0..len {
                    x.push(Decode::decode(decoder)?).unwrap();
                }
                x
            }
        })
    }
}

impl ScopePacket {
    pub fn new(id: u32, x: &Container, getters: &[<Container as RemoteGet>::GetterType]) -> ScopePacket {
        let mut buf = Vec::new();
        buf.resize(buf.capacity(), 0).unwrap();

        let mut length = 0;
        let mut probe_valid = 0;

        for (idx, probe) in getters.iter().enumerate() {
            if let Ok(value) = x.get((*probe).clone()) {
                if let Some(field_length) = value.dehydrate(&mut buf[length..]) {
                    length += field_length;
                    probe_valid.bit_set(idx);
                }
            }
        };

        buf.truncate(length);

        ScopePacket {
            id,
            probe_valid,
            buf
        }
    }

    pub fn rehydrate(&self, getters: &[<Container as RemoteGet>::GetterType]) -> Vec<Option<<Container as RemoteGet>::ValueType>, SCOPE_PROBES> {
        assert!(getters.len() <= SCOPE_PROBES);

        let mut ret = Vec::new();
        let mut offset = 0;
        for (idx, probe) in getters.iter().enumerate() {
            ret.push(
                if self.probe_valid.bit_test(idx) {
                    if let Ok((value, field_length)) = <Container as RemoteGet>::hydrate((*probe).clone(), &self.buf[offset..]) {
                        offset += field_length;
                        Some(value)
                    } else {
                        None
                    }
                } else {
                    None
                }).unwrap()
        }
        ret
    }
}

#[derive(Encode, Decode, Debug)]
pub enum DeviceToHost {
    Sample(ScopePacket),
    SetterReply(Result<(), ()>),
    GetterReply(Result<CValue, ()>)
}

#[derive(Encode, Decode, Debug)]
pub enum HostToDevice {
    AddProbe(CGetter),
    ClearProbes,
    ProbeInterval(u32),
    Setter(CSetter),
    Getter(CGetter)
}

#[derive(RemoteGetter, RemoteSetter, Debug)]
#[remote(derive(Encode, Decode, Debug))]
pub struct Container<'a> {
    #[remote(read_only)]
    pub adc: &'a [u16; 16],
    #[remote(read_only)]
    pub pwm: &'a [u16; 3],
    pub controller: &'a mut foc::state_machine::Controller,
    #[remote(read_only)]
    pub update: &'a ControllerUpdate,
    #[remote(read_only)]
    pub encoder: &'a EncoderState,
}

type CGetter = <Container<'static> as RemoteGet>::GetterType;
type CValue = <Container<'static> as RemoteGet>::ValueType;

type CSetter = <Container<'static> as RemoteSet>::SetterType;

pub fn to_controller_update(adc_buf: &[u16; 16], position: Option<EncoderOutput>, config: &Config) -> ControllerUpdate {
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
        position: position.map(|x| x.multiply(config.encoder_len_per_cycle / core::f32::consts::TAU))
    }
}