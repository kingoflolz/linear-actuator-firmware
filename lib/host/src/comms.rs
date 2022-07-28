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
        let reader = BufReader::with_capacity(1024, Handle(self.device_handle.clone()));
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
    // device_handle.claim_interface(1).unwrap();
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

pub fn new_interface() -> (Sender<HostToDevice>, Receiver<DeviceToHost>, Receiver<ScopePacket>) {
    let (writer_send, reader_recv) = new_device_pair();
    let (scope_send, scope_recv) = channel();
    let (reader_recv_fwd_send, reader_recv_fwd_recv) = channel();

    spawn(move || {
        for d2h in reader_recv {
            match d2h {
                DeviceToHost::Sample(s) => {
                    scope_send.send(s).unwrap();
                }
                other @ _ => {
                    reader_recv_fwd_send.send(other).unwrap();
                }
            }
        };
    });

    (writer_send, reader_recv_fwd_recv, scope_recv)
}