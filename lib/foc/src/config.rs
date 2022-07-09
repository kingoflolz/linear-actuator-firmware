pub struct Config {
    // encoder calibration
    pub calibration_length: f32, // in electrical revolutions
    pub calibration_speed: f32, // in electrical revolutions per second
    pub calibration_voltage: f32, // in volts

    pub uvlo: f32, // in volts

    pub switching_frequency: f32, // in Hz
    pub control_frequency: f32
}

impl Config {
    pub fn new() -> Self {
        Config {
            calibration_length: 10.0,
            calibration_speed: 1.0,
            calibration_voltage: 3.0,
            uvlo: 10.0,
            switching_frequency: 200e3,
            control_frequency: 8e3,
        }
    }
}