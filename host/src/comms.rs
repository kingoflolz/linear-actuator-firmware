use std::io::{BufReader, BufWriter, Read, Write};
use std::time::Duration;

use std::sync::{Arc, mpsc};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread::spawn;
use eventuals::Eventual;

use rusb::{DeviceHandle, GlobalContext, open_device_with_vid_pid};
use common::*;
use remote_obj::prelude::*;

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
    match device_handle.kernel_driver_active(1) {
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


pub enum ArbiterReq {
    Getter(ContainerGetter, Sender<Result<ContainerValue, ()>>),
    Setter(ContainerSetter, Sender<Result<(), ()>>),
    Other(HostToDevice)
}

impl ArbiterReq {
    pub fn get(getter: ContainerGetter, sender: &Sender<ArbiterReq>) -> Result<f32, ()> {
        let timeout = Duration::from_millis(1000);
        let (s, r) = channel();
        let req = ArbiterReq::Getter(getter, s);
        sender.send(req).unwrap();
        r.recv_timeout(timeout).unwrap().map(|x| x.as_float().unwrap())
    }

    pub fn set_async(setter: ContainerSetter, sender: &Sender<ArbiterReq>) {
        let (s, r) = channel();
        let req = ArbiterReq::Setter(setter, s);
        sender.send(req).unwrap();
    }

    pub fn other(x: HostToDevice, sender: &Sender<ArbiterReq>) {
        match x {
            HostToDevice::AddProbe(_) | HostToDevice::ClearProbes | HostToDevice::ProbeInterval(_) => {}
            _ => unreachable!()
        }
        sender.send(ArbiterReq::Other(x)).unwrap();
    }
}

pub struct Arbiter {
    cmd_s: Sender<HostToDevice>,
    cmd_r: Receiver<DeviceToHost>,

    receiver: Receiver<ArbiterReq>
}

impl Arbiter {
    pub fn start(cmd_s: Sender<HostToDevice>, cmd_r: Receiver<DeviceToHost>) -> Sender<ArbiterReq> {
        let (sender, receiver) = channel();
        Arbiter {
            cmd_s,
            cmd_r,
            receiver,
        }.run();

        sender
    }

    fn run(self) {
        spawn(move || {
            let timeout = Duration::from_millis(1000);
            for req in self.receiver {
                match req {
                    ArbiterReq::Getter(g, reply) => {
                        self.cmd_s.send(HostToDevice::Getter(g)).unwrap();
                        let r = self.cmd_r.recv_timeout(timeout).unwrap();
                        match r {
                            DeviceToHost::GetterReply(r) => {
                                let _ = reply.send(r);
                            }
                            _ => unreachable!()
                        }
                    }
                    ArbiterReq::Setter(s, reply) => {
                        self.cmd_s.send(HostToDevice::Setter(s)).unwrap();
                        let r = self.cmd_r.recv_timeout(timeout).unwrap();
                        match r {
                            DeviceToHost::SetterReply(r) => {
                                let _ = reply.send(r);
                            }
                            _ => unreachable!()
                        }
                    }
                    ArbiterReq::Other(o) => {
                        self.cmd_s.send(o).unwrap();
                    }
                }
            }
        });
    }
}

pub fn new_interface() -> (Sender<ArbiterReq>, Receiver<DeviceToHost>) {
    let (writer_send, reader_recv) = new_device_pair();
    let (scope_send, scope_recv) = channel();
    let (reader_recv_fwd_send, reader_recv_fwd_recv) = channel();

    spawn(move || {
        for d2h in reader_recv {
            match d2h {
                DeviceToHost::Sample(_) |
                DeviceToHost::ProbeAdded |
                DeviceToHost::ProbeRemoved |
                DeviceToHost::ProbeCleared => {
                    scope_send.send(d2h).unwrap();
                }
                _ => {
                    reader_recv_fwd_send.send(d2h).unwrap();
                }
            }
        };
    });

    (Arbiter::start(writer_send, reader_recv_fwd_recv), scope_recv)
}

pub struct CachedGetterSetter {
    getter: ContainerGetter,
    setter: Box<dyn Fn(f32) -> ContainerSetter>,
    cached_value: f32,
    arb: Sender<ArbiterReq>
}

impl CachedGetterSetter {
    pub fn new(getter: ContainerGetter,
               setter: Box<dyn Fn(f32) -> ContainerSetter>,
               arb: Sender<ArbiterReq>) -> Option<Self> {
        let cached_value = ArbiterReq::get(getter, &arb).ok()?;
        Some(CachedGetterSetter {
            getter,
            setter,
            cached_value,
            arb
        })
    }

    pub fn getter_setter(&mut self) -> impl FnMut(Option<f64>) -> f64 + '_ {
        |x| {
            if let Some(x) = x {
                let setter = (self.setter)(x as f32);
                ArbiterReq::set_async(
                    setter,
                    &self.arb
                );
                self.cached_value = x as f32;
            };
            self.cached_value as f64
        }
    }
}


pub struct GetterSetter {
    getter: ContainerGetter,
    setter: Box<dyn Fn(f32) -> ContainerSetter>,
    value: Eventual<ContainerValue>,
    arb: Sender<ArbiterReq>
}


impl GetterSetter {
    pub fn new(getter: ContainerGetter,
               setter: Box<dyn Fn(f32) -> ContainerSetter>,
               value: Eventual<ContainerValue>,
               arb: Sender<ArbiterReq>) -> Option<Self> {
        Some(GetterSetter {
            getter,
            setter,
            value,
            arb
        })
    }

    pub fn getter_setter(&mut self) -> Option<impl FnMut(Option<f64>) -> f64 + '_> {
        let value = self.value.value_immediate()?;

        Some(move |x| {
            if let Some(x) = x {
                let setter = (self.setter)(x as f32);
                ArbiterReq::set_async(
                    setter,
                    &self.arb
                );
            };
            value.as_float().unwrap() as f64
        })
    }
}
