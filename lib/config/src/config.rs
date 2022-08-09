use remote_obj::*;
use bincode::{Encode, Decode};

#[derive(RemoteSetter, RemoteGetter, Debug)]
#[remote(derive(Encode, Decode, Debug))]
pub struct Config {
    // setup constants
    pub motor_len_per_cycle: f32, // mm per electrical cycle
    pub encoder_len_per_cycle: f32, // mm per cycle

    // encoder calibration
    pub calibration_length: f32, // in mm
    pub calibration_speed: f32, // in electrical revolutions per second
    pub open_loop_voltage: f32, // in volts

    pub uvlo: f32, // in volts

    pub switching_frequency: f32, // in Hz
    pub switching_clock_frequency: f32,
    /// how much to switch all 3 phases to all-on for bootstrap cap recharge
    pub cycle_deadtime: f32, // in seconds
    pub control_frequency: f32,

    pub current_controller_k_p: f32,
    pub current_controller_k_i: f32,

    pub vel_controller_k_p: f32,
    pub vel_controller_k_i: f32,
    pub pos_controller_k_p: f32,

    pub curr_limit: f32,
    pub hard_curr_limit: f32,

    pub comp_matrix: [[f32; 8]; 8],
    pub comp_bias: [f32; 8]
}

impl Config {
    pub fn new() -> Self {
        Config {
            motor_len_per_cycle: 19.0,
            encoder_len_per_cycle: 2.34375,
            calibration_length: 100.0,
            calibration_speed: 0.1,
            open_loop_voltage: 0.5,
            uvlo: 10.0,
            switching_frequency: 200e3,
            switching_clock_frequency: 100e6,
            cycle_deadtime: 300e-9, // ~50ns is min controllable on time
            control_frequency: 8e3,

            current_controller_k_p: 0.22e-4,
            current_controller_k_i: 1000.0 * 60e-3 / 8e3,
            vel_controller_k_p: 0.1,
            vel_controller_k_i: 10.0 / 8e3,
            pos_controller_k_p: 40.0,

            curr_limit: 22.5,
            hard_curr_limit: 35.0,
            comp_matrix: [
                [ 1.1173, -0.8311,  0.2963, -0.3230,  0.0120,  0.0302,  0.0000,  0.0000],
                [-1.0591,  0.9015, -0.3551,  0.2872, -0.0080, -0.0282,  0.0000,  0.0000],
                [ 0.3346, -0.3011,  0.7365, -0.7179,  0.0443,  0.0641,  0.0000,  0.0000],
                [-0.4202,  0.1919, -0.6281,  0.7852, -0.0504, -0.0697,  0.0000,  0.0000],
                [-0.0926,  0.0768, -0.0518,  0.0827,  1.0994, -0.2049, -0.1680, -0.1636],
                [-0.0895, -0.2740, -0.1790, -0.3050, -0.1038,  1.2927,  0.0032, -0.0015],
                [ 0.0000,  0.0000,  0.0000,  0.0000,  0.0084, -0.0191,  1.3382, -0.2344],
                [ 0.0000,  0.0000,  0.0000,  0.0000, -0.0405,  0.1293, -0.2132,  1.4222]
            ],
            comp_bias: [-0.0095, -0.0527, -0.0642, -0.0139, -0.0410, -0.0730, -0.0624, -0.0489]
        }
    }
}