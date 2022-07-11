use serialport;
use framed;
use common::Sample;
use bincode;

fn main() {
    // let ports = serialport::available_ports().unwrap();
    //
    // for port in ports {
    //     println!("{}", port.port_name);
    // }

    let bincode_config = bincode::config::standard()
        .with_little_endian()
        .with_fixed_int_encoding()
        .skip_fixed_array_length();

    let mut codec = framed::bytes::Config::default();

    let port = serialport::new("/dev/ttyACM0", 9600).open().unwrap();

    let mut receiver = codec.to_receiver(port);

    let mut last_id: u16 = 0;

    loop {
        let frame = receiver.recv();
        match frame {
            Ok(frame) => {
                let sample: Sample = bincode::decode_from_slice(
                    &frame,
                    bincode_config,
                ).unwrap().0;
                if sample.id != last_id.wrapping_add(1) {
                    println!("sample mismatch: {} follows {}", sample.id, last_id);
                }
                last_id = sample.id;
                if sample.id % 1000 == 0 {
                    println!("{:?}", sample);
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
