use serialport;
use framed;
use common::{Sample, to_controller_update};
use foc::config::Config;
use bincode;
extern crate npy;

fn main() {
    // let ports = serialport::available_ports().unwrap();
    //
    // for port in ports {
    //     println!("{}", port.port_name);
    // }

    let mut arr: Vec<f32> = Vec::new();

    let bincode_config = bincode::config::standard()
        .with_little_endian()
        .with_fixed_int_encoding()
        .skip_fixed_array_length();

    let mut codec = framed::bytes::Config::default();

    let port = serialport::new("/dev/ttyACM21", 9600).open().unwrap();

    let mut receiver = codec.to_receiver(port);

    let mut last_id: u16 = 0;

    let config = Config::new();

    loop {
        let frame = receiver.recv();
        match frame {
            Ok(frame) => {
                let sample: Sample = bincode::decode_from_slice(
                    &frame,
                    bincode_config,
                ).unwrap().0;
                // if sample.id != last_id.wrapping_add(4) {
                //     println!("sample mismatch: {} follows {}", sample.id, last_id);
                // }
                last_id = sample.id;

                let update =  to_controller_update(&sample.adc, &None, &config);

                if sample.id % 500 == 0 {
                    // println!("pos: {:?}mm, pos tgt: {}, calib {:?}", sample.position, sample.position_target, sample.calibration);
                    println!("{:?} {:?}", sample.pwm, update.phase_currents);
                }
                for i in sample.adc.iter() {
                    arr.push(*i as f32);
                }
                if arr.len() > 1000000 {
                    npy::to_file("save.npy", arr).unwrap();
                    break
                }
            },
            Err(framed::Error::Io(e)) => {
                if e.kind() != std::io::ErrorKind::TimedOut {
                    println!("breaking from io error: {:?}", e);
                    break
                }
            },
            _ => {}
        }
    }

}
