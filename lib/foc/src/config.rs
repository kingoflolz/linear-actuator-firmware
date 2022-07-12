pub struct Config {
    // encoder calibration
    pub calibration_length: f32, // in electrical revolutions
    pub calibration_speed: f32, // in electrical revolutions per second
    pub calibration_voltage: f32, // in volts

    pub uvlo: f32, // in volts

    pub switching_frequency: f32, // in Hz
    pub switching_clock_frequency: f32,
    /// how much to switch all 3 phases to all-on for bootstrap cap recharge
    pub cycle_deadtime: f32, // in seconds
    pub control_frequency: f32
}

impl Config {
    pub fn new() -> Self {
        Config {
            calibration_length: 15.0,
            calibration_speed: 1.0,
            calibration_voltage: 1.5,
            uvlo: 10.0,
            switching_frequency: 200e3,
            switching_clock_frequency: 100e6,
            cycle_deadtime: 50e-9, // ~50ns is min controllable on time
            control_frequency: 8e3,
        }
    }
}