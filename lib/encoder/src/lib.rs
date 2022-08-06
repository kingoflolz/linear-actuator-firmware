#![no_std]

#[cfg(any(feature = "std", test))]
#[macro_use]
extern crate std;

use config::Config;
use nalgebra::{base, RowSVector, SMatrix, SVector};
use bincode::{Decode, Encode};

pub mod normalizer;
pub mod unwrap;
use biquad::*;

use remote_obj::prelude::*;

#[derive(RemoteGetter, RemoteSetter, Debug, Clone)]
#[remote(derive(Encode, Decode, Debug))]
pub struct EncoderCalibrator {
    normalizers: [normalizer::NormalizerBuilder; 8],
}

impl EncoderCalibrator {
    pub fn new() -> EncoderCalibrator {
        EncoderCalibrator {
            normalizers: [normalizer::NormalizerBuilder::new(); 8],
        }
    }

    pub fn update(&mut self, encoder_values: [f32; 8]) {
        for i in 0..8 {
            self.normalizers[i].update(encoder_values[i]);
        }
    }

    fn get_normalizers(&self) -> [normalizer::Normalizer; 8] {
        let mut normalizers = [normalizer::Normalizer::new(); 8];
        for i in 0..8 {
            normalizers[i] = self.normalizers[i].get_normalizer().unwrap();
        }
        normalizers
    }

    pub fn get_encoder(&self) -> Encoder {
        let f0 = 100.hz();
        let fs = 8.khz();
        let coeffs = Coefficients::<f32>::from_params(Type::LowPass, fs, f0, Q_BUTTERWORTH_F32).unwrap();
        Encoder {
            normalizers: self.get_normalizers(),
            unwraps: [unwrap::Unwrapper::new(); 4],
            normalized: [0.0; 8],
            compensated: [0.0; 8],
            unwrapped: [0.0; 4],
            position: 0.0,
            filtered_position: 0.0,
            velocity: 0.0,
            last_position: None,
            vel_filter: DirectForm1::<f32>::new(coeffs)
        }
    }
}

#[derive(RemoteGetter, RemoteSetter, Debug, Clone)]
#[remote(derive(Encode, Decode, Debug))]
pub struct Encoder {
    normalizers: [normalizer::Normalizer; 8],
    unwraps: [unwrap::Unwrapper; 4],
    normalized: [f32; 8],
    compensated: [f32; 8],
    unwrapped: [f32; 4],
    position: f32,
    filtered_position: f32,
    velocity: f32,
    #[remote(skip)]
    last_position: Option<f32>,
    #[remote(skip)]
    vel_filter: DirectForm1::<f32>,
}

#[derive(RemoteGetter, RemoteSetter, Default, Debug, Clone, PartialEq)]
#[remote(derive(Encode, Decode, Debug))]
pub struct EncoderOutput {
    pub position: f32,
    pub filtered_position: f32,
    pub velocity: f32,
}

impl EncoderOutput {
    pub fn multiply(&self, factor: f32) -> EncoderOutput {
        EncoderOutput {
            position: self.position * factor,
            filtered_position: self.filtered_position * factor,
            velocity: self.velocity * factor,
        }
    }
}

impl Encoder {
    pub fn calculate(&mut self, encoder_values: [f32; 8], config: &Config) -> EncoderOutput {
        for i in 0..encoder_values.len() {
            self.normalized[i] = self.normalizers[i].normalize(encoder_values[i])
        }

        let input_vec = RowSVector::<f32, 8>::from(self.normalized.clone());

        let weight_mat = SMatrix::<f32, 8, 8>::from(config.comp_matrix);

        let output = (input_vec + RowSVector::<f32, 8>::from(config.comp_bias)) * weight_mat;

        for i in 0..encoder_values.len() {
            self.compensated[i] = output[i]
        }

        let angle1 = libm::atan2f(self.normalized[0] - self.normalized[1], self.normalized[2] - self.normalized[3]);
        let angle2 = libm::atan2f(self.normalized[1], self.normalized[3]);
        let angle3 = libm::atan2f(self.normalized[4], self.normalized[5]);
        let angle4 = libm::atan2f(self.normalized[6], self.normalized[7]);

        let unwrap1 = self.unwraps[0].unwrap(angle1);
        let unwrap2 = self.unwraps[1].unwrap(angle2);
        let unwrap3 = self.unwraps[2].unwrap(angle3);
        let unwrap4 = self.unwraps[3].unwrap(angle4);

        self.unwrapped[0] = unwrap1;
        self.unwrapped[1] = unwrap2;
        self.unwrapped[2] = unwrap3;
        self.unwrapped[3] = unwrap4;

        self.position = unwrap1;

        self.filtered_position = self.vel_filter.run(self.position);

        if let Some(last_pos) = self.last_position {
            self.velocity = (self.filtered_position - last_pos) * 8e3;
        } else {
            self.velocity = 0.0;
        }

        self.last_position = Some(self.filtered_position);

        EncoderOutput{
            position: unwrap1,
            filtered_position: self.filtered_position,
            velocity: self.velocity
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

    pub fn update(&mut self, encoder_values: [f32; 8], config: &Config) -> Option<EncoderOutput> {
        match self {
            EncoderState::Calibrating(calibrator) => {
                calibrator.update(encoder_values);
                None
            }
            EncoderState::Running(encoder) => {
                Some(encoder.calculate(encoder_values, config))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linalg() {
        let input_vec = RowSVector::<f32, 8>::from([-0.2862,  0.3974,  0.6592, -0.9261,  0.6866, -0.2952, -0.3340, -0.7781]);

        let weight_mat = SMatrix::<f32, 8, 8>::from([[ 1.1173, -0.8311,  0.2963, -0.3230,  0.0120,  0.0302,  0.0000,  0.0000],
            [-1.0591,  0.9015, -0.3551,  0.2872, -0.0080, -0.0282,  0.0000,  0.0000],
            [ 0.3346, -0.3011,  0.7365, -0.7179,  0.0443,  0.0641,  0.0000,  0.0000],
            [-0.4202,  0.1919, -0.6281,  0.7852, -0.0504, -0.0697,  0.0000,  0.0000],
            [-0.0926,  0.0768, -0.0518,  0.0827,  1.0994, -0.2049, -0.1680, -0.1636],
            [-0.0895, -0.2740, -0.1790, -0.3050, -0.1038,  1.2927,  0.0032, -0.0015],
            [ 0.0000,  0.0000,  0.0000,  0.0000,  0.0084, -0.0191,  1.3382, -0.2344],
            [ 0.0000,  0.0000,  0.0000,  0.0000, -0.0405,  0.1293, -0.2132,  1.4222]]);

        let output = (input_vec + RowSVector::<f32, 8>::from([-0.0047, -0.0263, -0.0321, -0.0069, -0.0205, -0.0365, -0.0312, -0.0245])) * weight_mat;

        println!("{:?}", output)
    }
}