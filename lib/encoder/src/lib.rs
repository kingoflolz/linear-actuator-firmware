#![no_std]

#[cfg(any(feature = "std", test))]
#[macro_use]
extern crate std;

pub mod normalizer;
pub mod unwrap;

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
            normalizers[i] = self.normalizers[i].get_normalizer();
        }
        normalizers
    }

    pub fn get_encoder(&self) -> Encoder {
        Encoder {
            normalizers: self.get_normalizers(),
            unwraps: [unwrap::Unwrapper::new(); 4],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Encoder {
    normalizers: [normalizer::Normalizer; 4],
    unwraps: [unwrap::Unwrapper; 4],
}

impl Encoder {
    pub fn calculate(&mut self, encoder_values: [f32; 4]) -> (f32, [f32; 4]) {
        let [a, b, c, d] = encoder_values;

        let a = self.normalizers[0].normalize(a);
        let b = self.normalizers[1].normalize(b);
        let c = self.normalizers[2].normalize(c);
        let d = self.normalizers[3].normalize(d);

        let angle1 = libm::atan2f(d, a);
        let angle2 = libm::atan2f(c, b);
        let angle3 = libm::atan2f(a, c);
        let angle4 = libm::atan2f(b, d);

        ([angle1, angle2, angle4].iter().zip(&mut self.unwraps).map(|(angle, unwrap)| {
            unwrap.unwrap(*angle)
        }).sum::<f32>() / 3.0, [angle1, angle2, angle3, angle4])
    }
}

pub enum EncoderState {
    Calibrating(EncoderCalibrator),
    Running(Encoder),
}

impl EncoderState {
    pub fn new() -> EncoderState {
        EncoderState::Calibrating(EncoderCalibrator::new())
    }

    pub fn update(&mut self, encoder_values: [f32; 4]) -> Option<(f32, [f32; 4])> {
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