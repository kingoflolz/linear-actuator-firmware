use usbd_serial::CdcAcmClass;
use usb_device::prelude::*;
use bbqueue::{BBBuffer, Consumer, Producer};
use remote_obj::prelude::*;
use crate::app::UsbBusType;
use heapless::{Vec, spsc::{Consumer as SpscConsumer, Producer as SpscProducer}};
use heapless::spsc::Queue;
use rtt_target::rprintln;
use common::{Container, ContainerGetter, DeviceToHost, HostToDevice, ScopePacket, BINCODE_CFG, SCOPE_PROBES};

pub struct ControllerComms<const SEND_BUF: usize, const RECV_BUF: usize> {
    send_p: Producer<'static, SEND_BUF>,
    recv_c: SpscConsumer<'static, HostToDevice, RECV_BUF>,
    sample_id: u32,
    probes: Vec<ContainerGetter, SCOPE_PROBES>,
}

fn encode_and_frame(x: DeviceToHost, buf: &mut [u8]) -> usize {
    let local_buf = &mut [0; 128];
    let length = bincode::encode_into_slice(
        x,
        local_buf,
        BINCODE_CFG
    ).unwrap();

    let mut codec = framed::bytes::Config::default().to_codec();
    let encoded_len = codec.encode_to_slice(&local_buf[0..length], buf).unwrap();
    return encoded_len
}

impl<const SEND_BUF: usize, const RECV_BUF: usize> ControllerComms<SEND_BUF, RECV_BUF> {
    pub fn tick(&mut self, x: &mut Container) -> Result<(), ()> {
        self.sample_id += 1;

        if let Ok(mut grant) = self.send_p.grant_exact(64) {
            match self.recv_c.dequeue() {
                None => {}
                Some(host_command) => {
                    match host_command {
                        HostToDevice::AddProbe(p) => {
                            let _ = self.probes.push(p);
                        }
                        HostToDevice::ClearProbes => {
                            self.probes.clear()
                        }
                        HostToDevice::Setter(s) => {
                            let set_result = DeviceToHost::SetterReply(x.set(s));
                            let length = encode_and_frame(set_result, grant.buf());
                            grant.commit(length);
                        }
                        HostToDevice::Getter(g) => {
                            let get_result = DeviceToHost::GetterReply(x.get(g));
                            let length = encode_and_frame(get_result, grant.buf());
                            grant.commit(length);
                        }
                    }
                }
            }
        }

        let packet = DeviceToHost::Sample(ScopePacket::new(self.sample_id, x, &self.probes));

        let mut buf = [0; 128];
        let length = bincode::encode_into_slice(
            packet,
            &mut buf,
            BINCODE_CFG
        ).unwrap();

        match self.send_p.grant_exact(framed::bytes::max_encoded_len(length)) {
            Ok(mut g) => {
                let mut codec = framed::bytes::Config::default().to_codec();
                let encoded_len = codec.encode_to_slice(&buf[0..length], g.buf()).unwrap();
                g.commit(encoded_len);
                Ok(())
            }
            Err(_) => {
                Err(())
            }
        }
    }
}

pub struct USBCommunicator<const SEND_BUF: usize, const RECV_BUF: usize> {
    cdc: CdcAcmClass<'static, UsbBusType>,
    bus: UsbDevice<'static, UsbBusType>,
    send_c: Consumer<'static, SEND_BUF>,
    recv_p: SpscProducer<'static, HostToDevice, RECV_BUF>,
}

impl<const SEND_BUF: usize, const RECV_BUF: usize> USBCommunicator<SEND_BUF, RECV_BUF> {
    pub fn run(&mut self) {
        let mut buf: Vec<u8, 128> = Vec::new();
        let mut codec = framed::bytes::Config::default().to_codec();

        loop {
            self.bus.poll(&mut [&mut self.cdc]);
            match self.send_c.read() {
                Ok(g) => {
                    let write_length = g.len().min(self.cdc.max_packet_size() as usize);

                    match self.cdc.write_packet(&g.buf()[..write_length]) {
                        Ok(_) => {
                            g.release(write_length)
                        }
                        Err(_) => {}
                    }
                }
                Err(_) => {}
            }
            if (buf.len() + self.cdc.max_packet_size() as usize) < buf.capacity() {
                let packet_buffer = &mut [0; 64];
                match self.cdc.read_packet(packet_buffer) {
                    Ok(read_length) => {
                        buf.extend_from_slice(&packet_buffer[..read_length]).unwrap();
                    }
                    Err(_) => {}
                };
            }
            if self.recv_p.ready() {
                let mut consumed_length = 0;
                let mut prev_packet = 0;
                for (idx, i) in buf.iter().enumerate() {
                    if !self.recv_p.ready() {
                        break
                    }
                    if *i == 0x00 {
                        let packet = &buf[prev_packet..=idx];
                        prev_packet = idx;

                        let mut decode_buffer = [0; 128];
                        let codec_out = codec.decode_to_slice(packet, &mut decode_buffer);

                        let decoded_length;
                        match codec_out {
                            Ok(x) => {
                                decoded_length = x
                            }
                            Err(_) => {
                                continue
                            }
                        }

                        let packet;
                        match bincode::decode_from_slice(
                            &decode_buffer[..decoded_length],
                            BINCODE_CFG
                        ) {
                            Ok((x, _)) => {
                                packet = x
                            }
                            Err(_) => {
                                continue
                            }
                        }

                        // this must succeed as guarded by .ready before
                        self.recv_p.enqueue(packet).unwrap();
                    }
                }

                if consumed_length == 0 { // something's funky, reset and try again
                    buf.clear();
                } else{
                    buf = Vec::from_slice(&buf[prev_packet..]).unwrap();
                }
            }
        }
    }
}

pub fn get_comms_pair<'a, const SEND_BUF: usize, const RECV_BUF: usize>(
    bbq: &'static mut BBBuffer<SEND_BUF>,
    q: &'static mut Queue<HostToDevice, RECV_BUF>,
    cdc: CdcAcmClass<'static, UsbBusType>,
    bus: UsbDevice<'static, UsbBusType>)
    -> (ControllerComms<SEND_BUF, RECV_BUF>, USBCommunicator<SEND_BUF, RECV_BUF>) {
    let (s_p, s_c) = bbq.try_split().unwrap();
    let (r_p, r_c) = q.split();
    (
        ControllerComms {
            send_p: s_p,
            recv_c: r_c,
            sample_id: 0,
            probes: Vec::new(),
        },
        USBCommunicator {
            cdc,
            bus,
            send_c: s_c,
            recv_p: r_p,
        },
    )
}



