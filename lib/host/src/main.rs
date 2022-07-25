use std::io::{BufReader, Read};
use std::time::Duration;
use egui::remap_clamp;
use framed::bytes::Receiver;

use rusb::{Context, Device, DeviceDescriptor, DeviceHandle, Direction, Result, TransferType, UsbContext};
use common::Sample;

#[derive(Debug)]
struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8,
}

struct USBCDC {
    context: Context,
    device_handle: DeviceHandle<Context>,
}

impl USBCDC {
    pub fn new() -> Result<Self> {
        let vid = 0x16c0;
        let pid = 0x27dd;

        let mut context = Context::new()?;
        let mut device_handle = open_device(&mut context, vid, pid)?;
        device_handle.reset()?;

        let has_kernel_driver = match device_handle.kernel_driver_active(1) {
            Ok(true) => {
                device_handle.detach_kernel_driver(1).ok();
                true
            }
            _ => false,
        };

        device_handle.claim_interface(1).unwrap();

        Ok(USBCDC {
            context,
            device_handle
        })
    }
}

impl Read for USBCDC {
    fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
        let timeout = Duration::from_secs(1);

        self.device_handle.read_bulk(130, &mut buf, timeout).map_err(|x| std::io::ErrorKind::Other.into())
    }
}

struct PacketReader {
    reader: Receiver<BufReader<USBCDC>>
}

impl PacketReader {
    fn new() -> Self {
        let mut codec = framed::bytes::Config::default();
        let reader = BufReader::with_capacity(16384, USBCDC::new().unwrap());
        let receiver = codec.to_receiver(reader);

        PacketReader {
            reader: receiver
        }
    }

    fn read(&mut self) -> Sample {
        let bincode_config = bincode::config::standard()
            .with_little_endian()
            .with_fixed_int_encoding()
            .skip_fixed_array_length();

        loop {
            match self.reader.recv() {
                Ok(frame) => {
                    let sample: Sample = bincode::decode_from_slice(
                        &frame,
                        bincode_config,
                    ).unwrap().0;
                    return sample
                }
                Err(_) => {}
            }
        }
    }
}

fn main() {
    let mut p = PacketReader::new();
    let mut old_sample: u16 = 0;
    for i in 0..100000 {
        let s = p.read();
        if old_sample.wrapping_add(1) != s.id {
            println!("{} mismatch old: {}, new: {}", i, old_sample, s.id);
        }
        old_sample = s.id;
        // println!("{}", i)
    }
}

fn open_device<T: UsbContext>(
    context: &mut T,
    vid: u16,
    pid: u16,
) -> Result<DeviceHandle<T>> {
    let devices = context.devices()?;

    for device in devices.iter() {
        let device_desc = device.device_descriptor()?;

        if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
            return Ok(device.open()?);
        }
    }

    panic!("device not found")
}
