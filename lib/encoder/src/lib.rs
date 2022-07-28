#![no_std]

#[cfg(any(feature = "std", test))]
#[macro_use]
extern crate std;

use bincode::{Decode, Encode};

pub mod normalizer;
pub mod unwrap;

use remote_obj::prelude::*;

#[derive(RemoteGetter, RemoteSetter, Debug, Clone)]
#[remote(derive(Encode, Decode, Debug))]
pub struct EncoderCalibrator {
    normalizers: [normalizer::NormalizerBuilder; 4],
}

impl EncoderCalibrator {
    pub fn new() -> EncoderCalibrator {
        EncoderCalibrator {
            normalizers: [normalizer::NormalizerBuilder::new(); 4],
        }
    }

    pub fn update(&mut self, encoder_values: [f32; 4]) {
        for i in 0..4 {
            self.normalizers[i].update(encoder_values[i]);
        }
    }

    fn get_normalizers(&self) -> [normalizer::Normalizer; 4] {
        let mut normalizers = [normalizer::Normalizer::new(); 4];
        for i in 0..4 {
            normalizers[i] = self.normalizers[i].get_normalizer().unwrap();
        }
        normalizers
    }

    pub fn get_encoder(&self) -> Encoder {
        Encoder {
            normalizers: self.get_normalizers(),
            unwraps: [unwrap::Unwrapper::new(); 1],
            normalized: [0.0; 4],
            position: 0.0
        }
    }
}

#[derive(RemoteGetter, RemoteSetter, Debug, Clone, Default)]
#[remote(derive(Encode, Decode, Debug))]
pub struct Encoder {
    normalizers: [normalizer::Normalizer; 4],
    unwraps: [unwrap::Unwrapper; 1],
    normalized: [f32; 4],
    position: f32
}

#[derive(Encode, Decode, Debug, Clone, PartialEq)]
pub struct EncoderOutput {
    pub position: f32
}

impl Encoder {
    pub fn calculate(&mut self, encoder_values: [f32; 4]) -> EncoderOutput {
        let [a, b, c, d] = encoder_values;

        let a = self.normalizers[0].normalize(a);
        let b = self.normalizers[1].normalize(b);
        let c = self.normalizers[2].normalize(c);
        let d = self.normalizers[3].normalize(d);

        self.normalized = [a, b, c, d];

        let x = a - b;
        let y = c - d;

        let angle1 = libm::atan2f(x, y);

        let unwrap1 = self.unwraps[0].unwrap(angle1);
        self.position = unwrap1;

        EncoderOutput{
            position: unwrap1,
        }
    }
}

#[derive(RemoteGetter, RemoteSetter, Debug, Clone)]
#[remote(derive(Encode, Decode, Debug))]
pub enum EncoderState {
    Calibrating(EncoderCalibrator),
    Running(Encoder),
}

impl EncoderState {
    pub fn new() -> EncoderState {
        EncoderState::Calibrating(EncoderCalibrator::new())
    }

    pub fn update(&mut self, encoder_values: [f32; 4]) -> Option<EncoderOutput> {
        match self {
            EncoderState::Calibrating(calibrator) => {
                calibrator.update(encoder_values);
                None
            }
            EncoderState::Running(encoder) => {
                Some(encoder.calculate(encoder_values))
            }
        }
    }

    pub fn calibration_done(&mut self) {
        match self {
            EncoderState::Calibrating(calibrator) => {
                *self = EncoderState::Running(calibrator.get_encoder());
            }
            _ => {}
        }
    }
}