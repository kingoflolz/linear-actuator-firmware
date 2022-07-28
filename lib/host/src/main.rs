use std::io::{BufReader, BufWriter, IoSlice, Read, Write};
use std::time::Duration;
use egui::remap_clamp;

use core::hash::Hasher;
use std::fmt::Arguments;
use std::sync::{Arc, mpsc};
use std::sync::mpsc::{channel, Sender, Receiver, RecvTimeoutError};
use std::thread::spawn;
use remote_obj::getter;

use rusb::{Context, Device, DeviceDescriptor, DeviceHandle, Direction, GlobalContext, open_device_with_vid_pid, Result, TransferType, UsbContext};
use common::{BINCODE_CFG, DeviceToHost, HostToDevice, ScopePacket, ContainerGetter};


struct DeviceReader {
    device_handle: Arc<DeviceHandle<GlobalContext>>,
    channel: Sender<DeviceToHost>,
}

impl DeviceReader {
    fn run(self) {
        struct Handle(Arc<DeviceHandle<GlobalContext>>);
        impl Read for Handle {
            fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
                let timeout = Duration::from_secs(1);
                self.0.read_bulk(130, &mut buf, timeout).map_err(|_| std::io::ErrorKind::Other.into())
            }
        }

        let mut codec = framed::bytes::Config::default();
        let reader = BufReader::with_capacity(16384, Handle(self.device_handle.clone()));
        let mut receiver = codec.to_receiver(reader);

        loop {
            match receiver.recv() {
                Ok(packet) => {
                    let decoded: DeviceToHost;
                    match bincode::decode_from_slice(
                        &packet,
                        BINCODE_CFG,
                    ) {
                        Ok((p, len)) => {
                            decoded = p;
                            assert_eq!(packet.len(), len);
                        }
                        Err(_) => {
                            println!("failed to decode packet {:?}", packet);
                            continue;
                        }
                    }
                    self.channel.send(decoded).unwrap();
                }
                Err(_) => {}
            }
        }
    }
}

struct DeviceWriter {
    device_handle: Arc<DeviceHandle<GlobalContext>>,
    channel: Receiver<HostToDevice>,
}

impl DeviceWriter {
    fn run(self) {
        struct Handle(Arc<DeviceHandle<GlobalContext>>);
        impl Write for Handle {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                let timeout = Duration::from_secs(1);
                let r = self.0.write_bulk(1, &buf, timeout).map_err(|_| std::io::ErrorKind::Other.into());
                println!("write done");
                r
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut codec = framed::bytes::Config::default();
        let writer = BufWriter::with_capacity(1024, Handle(self.device_handle.clone()));
        let mut sender = codec.to_sender(writer);

        let timeout = Duration::from_millis(1);

        loop {
            match self.channel.recv_timeout(timeout) {
                Ok(packet) => {
                    println!("got packet {:?}", packet);
                    let frame = bincode::encode_to_vec(
                        packet,
                        BINCODE_CFG,
                    ).unwrap();

                    sender.queue(&frame[..]).unwrap();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    sender.flush().unwrap();
                }
                Err(e) => {
                    println!("{:?}", e);
                    break;
                }
            }
        }
    }
}

pub fn new_device_pair() -> (Sender<HostToDevice>, Receiver<DeviceToHost>) {
    let vid = 0x1209;
    let pid = 0x0001;

    let mut device_handle = open_device_with_vid_pid(vid, pid).expect("device not found");
    device_handle.reset().unwrap();
    let has_kernel_driver = match device_handle.kernel_driver_active(1) {
        Ok(true) => {
            device_handle.detach_kernel_driver(1).ok();
            true
        }
        _ => false,
    };

    let device_handle = Arc::new(device_handle);

    let (reader_send, reader_recv) = channel();
    let (writer_send, writer_recv) = channel();

    let read_handle = device_handle.clone();
    spawn(move || {
        let reader = DeviceReader {
            device_handle: read_handle,
            channel: reader_send,
        };
        reader.run();
    });

    let write_handle = device_handle.clone();
    spawn(move || {
        let writer = DeviceWriter {
            device_handle: write_handle,
            channel: writer_recv,
        };
        writer.run();
    });
    (writer_send, reader_recv)
}


fn main() {
    let (s, r) = new_device_pair();

    let mut old_sample: u32 = 0;

    s.send(HostToDevice::ClearProbes);
    s.send(HostToDevice::AddProbe(getter!(Container.adc[0]))).unwrap();
    s.send(HostToDevice::AddProbe(getter!(Container.adc[1]))).unwrap();
    s.send(HostToDevice::AddProbe(getter!(Container.adc[2]))).unwrap();
    s.send(HostToDevice::AddProbe(getter!(Container.adc[3]))).unwrap();

    s.send(HostToDevice::Getter(getter!(Container.adc[0]))).unwrap();

    for i in 0..100000 {
        let s = r.recv().unwrap();
        if let DeviceToHost::Sample(s) = s {
            if old_sample.wrapping_add(1) != s.id {
                println!("{} mismatch old: {}, new: {}", i, old_sample, s.id);
            }
            old_sample = s.id;
            if i % 1000 == 0 {
                println!("{}, {}, {:b}", s.id, s.buf.len(), s.probe_valid);
            }
        } else {
            println!("{:?}", s);
        }
    }
}
