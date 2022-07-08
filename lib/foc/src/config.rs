pub struct Config {
    // encoder calibration
    pub calibration_length: f32, // in electrical revolutions
    pub calibration_speed: f32, // in electrical revolutions per second
    pub calibration_voltage: f32, // in volts

    // power stage
    pub switching_frequency: f32, // in Hz
}