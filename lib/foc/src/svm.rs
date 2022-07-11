use crate::state_machine::{LowLevelControllerOutput, PWMCommand};
use rtt_target::{self, rprintln};

// inputs are alpha and beta voltages as fraction of vbus, outputs are duty cycles
fn calculate_svm(alpha: f32, beta: f32) -> (f32, f32, f32, bool) {
    let t_a;
    let t_b;
    let t_c;
    let sextant;
    let one_by_sqrt3 = 0.57735026919f32;
    let two_by_sqrt3 = 1.15470053838f32;

    if beta >= 0.0f32 {
        if alpha >= 0.0f32 {
            //quadrant I
            if one_by_sqrt3 * beta > alpha {
                sextant = 2; //sextant v2-v3
            } else {
                sextant = 1; //sextant v1-v2
            }
        } else {
            //quadrant II
            if -one_by_sqrt3 * beta > alpha {
                sextant = 3; //sextant v3-v4
            } else {
                sextant = 2; //sextant v2-v3
            }
        }
    } else {
        if alpha >= 0.0f32 {
            //quadrant IV
            if -one_by_sqrt3 * beta > alpha {
                sextant = 5; //sextant v5-v6
            } else {
                sextant = 6; //sextant v6-v1
            }
        } else {
            //quadrant III
            if one_by_sqrt3 * beta > alpha {
                sextant = 4; //sextant v4-v5
            } else {
                sextant = 5; //sextant v5-v6
            }
        }
    }

    match sextant {
        // sextant v1-v2
        1 => {
            // Vector on-times
            let t1 = alpha - one_by_sqrt3 * beta;
            let t2 = two_by_sqrt3 * beta;

            // PWM timings
            t_a = 0.0;
            t_b = t_a + t1;
            t_c = t_b + t2;
        }

        // sextant v2-v3
        2 => {
            // Vector on-times
            let t2 = alpha + one_by_sqrt3 * beta;
            let t3 = -alpha + one_by_sqrt3 * beta;

            // PWM timings
            t_b = 0.0;
            t_a = t_b + t3;
            t_c = t_a + t2;
        }

        // sextant v3-v4
        3 => {
            // Vector on-times
            let t3 = two_by_sqrt3 * beta;
            let t4 = -alpha - one_by_sqrt3 * beta;

            // PWM timings
            t_b = 0.0;
            t_c = t_b + t3;
            t_a = t_c + t4;
        }

        // sextant v4-v5
        4 => {
            // Vector on-times
            let t4 = -alpha + one_by_sqrt3 * beta;
            let t5 = -two_by_sqrt3 * beta;

            // PWM timings
            t_c = 0.0;
            t_b = t_c + t5;
            t_a = t_b + t4;
        }

        // sextant v5-v6
        5 => {
            // Vector on-times
            let t5 = -alpha - one_by_sqrt3 * beta;
            let t6 = alpha - one_by_sqrt3 * beta;

            // PWM timings
            t_c = 0.0;
            t_a = t_c + t5;
            t_b = t_a + t6;
        }

        // sextant v6-v1
        6 => {
            // Vector on-times
            let t6 = -two_by_sqrt3 * beta;
            let t1 = alpha + one_by_sqrt3 * beta;

            // PWM timings
            t_a = 0.0;
            t_c = t_a + t1;
            t_b = t_c + t6;
        }

        _ => {
            // Should never happen
            t_a = 0.0f32;
            t_b = 0.0f32;
            t_c = 0.0f32;
        }
    }

    let result_valid = t_a >= 0.0f32
        && t_a <= 1.0f32
        && t_b >= 0.0f32
        && t_b <= 1.0f32
        && t_c >= 0.0f32
        && t_c <= 1.0f32;

    (t_a, t_b, t_c, result_valid)
}

fn round(x: f32) -> u16 {
    return (x + 0.5f32) as u16;
}

#[derive(Debug)]
pub struct IterativeSVM {
    pub residuals: [f32; 3],
    dead_time: u16, // dead time in duty cycle
    cycle_time: u16, // theoretical max modulation
}

impl IterativeSVM {
    pub fn new(dead_time: u16, cycle_time: u16) -> IterativeSVM {
        IterativeSVM {
            residuals: [0.0; 3],
            dead_time,
            cycle_time,
        }
    }

    // voltage in, duty cycle out
    pub fn calculate(&mut self, request: LowLevelControllerOutput) -> PWMCommand {
        // rprintln!("alpha: {}, beta: {}", request.alpha, request.beta);

        let (t_a, t_b, t_c, result_valid) = calculate_svm(request.alpha, request.beta);

        // rprintln!("ta = {}, tb = {}, tc = {}", t_a, t_b, t_c);

        let mut result_valid = result_valid && request.driver_enable;

        let t_a = t_a * self.cycle_time as f32;
        let t_b = t_b * self.cycle_time as f32;
        let t_c = t_c * self.cycle_time as f32;

        let t_a_rounded = round(t_a + self.residuals[0]);
        let t_b_rounded = round(t_b + self.residuals[1]);
        let t_c_rounded = round(t_c + self.residuals[2]);

        self.residuals[0] += t_a - t_a_rounded as f32;
        self.residuals[1] += t_b - t_b_rounded as f32;
        self.residuals[2] += t_c - t_c_rounded as f32;

        result_valid &= t_a_rounded + self.dead_time < self.cycle_time;
        result_valid &= t_b_rounded + self.dead_time < self.cycle_time;
        result_valid &= t_c_rounded + self.dead_time < self.cycle_time;

        use rtt_target::{self, rprintln};
        PWMCommand {
            driver_enable: result_valid,
            u_duty: t_a_rounded + self.dead_time,
            v_duty: t_b_rounded + self.dead_time,
            w_duty: t_c_rounded + self.dead_time
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_svm() {
        let (t_a, t_b, t_c, result_valid) = calculate_svm(1.0f32, 0.0f32);
        assert!(result_valid);

        let (t_a, t_b, t_c, result_valid) = calculate_svm(0.0f32, 1.0f32);
        assert!(!result_valid);
    }

    #[test]
    fn test_svm() {
        let mut svm = IterativeSVM::new(255);
        for _ in 0..100 {
            svm.calculate(1.0f32, 0.0f32);
        }
        assert!(svm.residuals.iter().all(|x| x.abs() < 1.0f32));
    }
}
