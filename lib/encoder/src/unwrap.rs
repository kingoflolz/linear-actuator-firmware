#[derive(Debug, Clone, Copy)]
pub struct Unwrapper {
    previous: f32
}

impl Unwrapper {
    pub fn new() -> Unwrapper {
        Unwrapper {
            previous: 0.0
        }
    }

    pub fn unwrap(&mut self, value: f32) -> f32 {
        let result = unwrap( self.previous, value);
        self.previous = result;
        result
    }
}

fn unwrap(previous_angle: f32, new_angle: f32) -> f32 {
    let pi = core::f32::consts::PI;
    let d = new_angle % (2.0 * pi) - previous_angle % (2.0 * pi);
    let offset = if d > pi {
        d - 2.0 * pi
    } else {
        if d < -pi {
            d + 2.0 * pi
        } else {
            d
        }
    };
    return previous_angle + offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap() {
        use rand_distr::{Uniform, Distribution};

        let normal = Uniform::new(-3.0, 3.0);
        let mut rng = rand::thread_rng();

        let mut gt_unwrapped = 0.0;
        let mut unwrapper = Unwrapper::new();

        for i in 0..10000 {
            let x = normal.sample(&mut rng);
            gt_unwrapped += x;

            let unwrapped = unwrapper.unwrap(gt_unwrapped % core::f32::consts::TAU);
            assert!((unwrapped - gt_unwrapped).abs() < 0.01, "{}, unwrapped: {}, gt_unwrapped: {}", i, unwrapped, gt_unwrapped);
        }
    }
}