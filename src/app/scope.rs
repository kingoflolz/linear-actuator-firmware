use usbd_serial::CdcAcmClass;
use usb_device::prelude::*;
use bbqueue::{BBBuffer, Producer, Consumer};
use bincode::Encode;
use remote_obj::Getter;
use crate::app::UsbBusType;
use core::marker::PhantomData;

pub struct ScopeProducer<const N: usize, T: Encode> {
    producer: Producer<'static, N>,
    sample_id: u32,
    dummy: PhantomData<T>
}

impl<const N: usize, T: Encode> ScopeProducer<N, T> where T: Encode {
    pub fn tick(&mut self, x: &T) -> bool {
        let mut buf = [0; 64];
        let length = bincode::encode_into_slice(
            x,
            &mut buf,
            bincode::config::standard()
                .with_little_endian()
                .with_fixed_int_encoding()
                .skip_fixed_array_length(),
        ).unwrap();

        match self.producer.grant_exact(framed::bytes::max_encoded_len(length)) {
            Ok(mut g) => {
                let mut codec = framed::bytes::Config::default().to_codec();
                let encoded_len = codec.encode_to_slice(&buf[0..length], g.buf()).unwrap();
                g.commit(encoded_len);
                true
            }
            Err(_) => {
                false
            }
        }
    }
}

pub struct ScopeConsumer<const N: usize> {
    cdc: CdcAcmClass<'static, UsbBusType>,
    bus: UsbDevice<'static, UsbBusType>,
    consumer: Consumer<'static, N>,
}

impl<const N: usize> ScopeConsumer<N> {
    pub fn run(&mut self) {
        loop {
            match self.consumer.read() {
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
            self.bus.poll(&mut [&mut self.cdc]);
        }
    }
}

pub fn get_scope<const N: usize, T: Encode>(bbq: &'static mut BBBuffer<N>, cdc: CdcAcmClass<'static, UsbBusType>, bus: UsbDevice<'static, UsbBusType>) -> (ScopeProducer<N, T>, ScopeConsumer<N>) {
    let (p, c) = bbq.try_split().unwrap();
    (
        ScopeProducer {
            producer: p,
            sample_id: 0,
            dummy: PhantomData::default()
        },
        ScopeConsumer {
            cdc,
            bus,
            consumer: c
        }
    )
}



