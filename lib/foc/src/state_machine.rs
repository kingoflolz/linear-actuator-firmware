pub struct LowLevelControllerOutput {
    pub driver_enable: bool,
    pub alpha: f32,
    pub beta: f32,
}

struct Controller {
    state: ControllerState,
}

enum ControllerState {
    Idle,
    Running,
    Fault,
}

pub struct PWMCommand {
    pub driver_enable: bool,
    pub u_duty: u16,
    pub v_duty: u16,
    pub w_duty: u16,
}

impl PWMCommand {
    pub fn to_array(&self) -> [u16; 3] {
        [self.u_duty, self.v_duty, self.w_duty]
    }
}

pub struct ControllerUpdate {
    pub u_current: f32,
    pub v_current: f32,
    pub w_current: f32,

    pub bus_voltage: f32,

    pub angle: Option<f32>,
}