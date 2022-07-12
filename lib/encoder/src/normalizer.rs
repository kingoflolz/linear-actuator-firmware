#[derive(Debug, Clone, Copy)]
pub struct NormalizerBuilder {
    n: u32,
    k: f32,
    ex: f32,
    ex2: f32,
}

impl NormalizerBuilder {
    pub fn new() -> NormalizerBuilder {
        NormalizerBuilder {
            n: 0,
            k: 0.0,
            ex: 0.0,
            ex2: 0.0,
        }
    }

    pub fn update(&mut self, x: f32) {
        if self.n == 0 {
            self.k = x;
        }
        self.n += 1;
        self.ex += x - self.k;
        self.ex2 += (x - self.k) * (x - self.k);
    }

    pub fn get_normalizer(&self) -> Normalizer {
        assert!(self.n > 2);

        let var = (self.ex2 - self.ex * self.ex / self.n as f32) / (self.n as f32 - 1.0);
        let std = libm::sqrtf(var);
        let mean = self.k + self.ex / self.n as f32;

        Normalizer {
            mean,
            std,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Normalizer{
    pub mean: f32,
    pub std: f32,
}

impl Normalizer {
    pub fn new() -> Normalizer {
        Normalizer {
            mean: 0.0,
            std: 0.0,
        }
    }

    pub fn normalize(&self, value: f32) -> f32 {
        (value - self.mean) / self.std
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_norm() {
        use rand_distr::{Normal, Distribution};

        let normal = Normal::new(5.0, 3.0).unwrap();
        let mut rng = rand::thread_rng();

        let mut b = NormalizerBuilder::new();

        for _ in 0..100000 {
            b.update(normal.sample(&mut rng))
        }

        let n = b.get_normalizer();

        assert!((n.mean - 5.0).abs() < 0.1);
        assert!((n.std - 3.0).abs() < 0.1);

        let mut b2 = NormalizerBuilder::new();

        for _ in 0..100000 {
            let normed = n.normalize(normal.sample(&mut rng));
            b2.update(normed);
        }

        let n = b2.get_normalizer();
        assert!(n.mean.abs() < 0.1);
        assert!((n.std - 1.0).abs() < 0.1);
    }
}