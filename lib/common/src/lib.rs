#![no_std]
use bincode::{Decode, Encode};

#[derive(Encode, Decode, Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub id: u16,
    pub adc: [u16; 10],
    pub pwm: [u16; 3]
}