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
}

impl Config {
    pub fn new() -> Self {
        Config {
            motor_len_per_cycle: 19.0,
            encoder_len_per_cycle: 2.34375,
            calibration_length: 100.0,
            calibration_speed: 1.0,
            open_loop_voltage: 2.0,
            uvlo: 10.0,
            switching_frequency: 200e3,
            switching_clock_frequency: 100e6,
            cycle_deadtime: 50e-9, // ~50ns is min controllable on time
            control_frequency: 8e3,

            current_controller_k_p: 0.0,
            current_controller_k_i: 0.0
        }
    }
}