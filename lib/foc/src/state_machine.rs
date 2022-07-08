use crate::calibration::EncoderCalibrationStatus;

struct Controller {
    state: ControllerState,
}

enum ControllerState {
    Idle,
    EncoderCalibration(EncoderCalibrationStatus),
    Running,
    Fault,
}

pub struct ControllerOutput {
    pub driver_enable: bool,
    pub alpha: f32,
    pub beta: f32,
}

pub struct ControllerUpdate {
    pub u_current: f32,
    pub v_current: f32,
    pub w_current: f32,

    pub bus_voltage: f32,

    pub angle: Option<f32>,
}