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

    let port = serialport::new("/dev/ttyAMC0", 9600).open().unwrap();

    let mut receiver = codec.to_receiver(port);

    loop {
        let frame = receiver.recv();
        match frame {
            Ok(frame) => {
                let sample: Sample = bincode::decode_from_slice(
                    &frame,
                    bincode::config::standard()
                        .with_little_endian()
                        .with_fixed_int_encoding()
                        .skip_fixed_array_length(),
                ).unwrap().0;
                println!("{:?}", sample);
            },
            Err(framed::Error::Io(_)) => {
                break
            },
            _ => {}
        }
    }

}
