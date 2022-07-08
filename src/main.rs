#![no_std]
#![no_main]

use panic_rtt_target as _;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [EXTI0])]
mod app {
    use core::convert::TryInto;
    use dwt_systick_monotonic::DwtSystick;
    use rtt_target::{self, rprintln, rtt_init_print};
    use stm32f4xx_hal::{
        adc::{
            Adc,
            config::{AdcConfig, Dma, SampleTime, Scan, Sequence}, Temperature,
        },
        dma::{config::DmaConfig, PeripheralToMemory, Stream0, StreamsTuple, Transfer},
        nb::block,
        otg_fs::{USB, UsbBus, UsbBusType},
        pac::{self, ADC1, DMA2},
        prelude::*,
        signature::{VtempCal110, VtempCal30},
        timer::{Event, Timer},
    };

    use foc::svm;

    use heapless::spsc::{Consumer, Producer, Queue};

    use dwt_systick_monotonic::fugit::Duration;
    use stm32f4xx_hal::adc::config::{Clock, ExternalTrigger, TriggerMode};
    use stm32f4xx_hal::pac::{TIM1, TIM2};

    use usb_device::bus::UsbBusAllocator;
    use usb_device::prelude::*;
    use usbd_serial::SerialPort;

    use foc::svm::IterativeSVM;
    use bincode::{config::*, Decode, Encode};
    use stm32f4xx_hal::hal::digital::v2::IoPin;
    use stm32f4xx_hal::timer::PwmChannel;

    use libm;

    #[derive(Encode)]
    pub struct Sample {
        magic: u16,
        id: u16,
        adc: [u16; 10],
        pwm: [u16; 3]
    }

    const MONO_HZ: u32 = 100_000_000;

    // 8 MHz
    #[monotonic(binds = SysTick, default = true)]
    type MyMono = DwtSystick<MONO_HZ>;

    type DMATransfer =
        Transfer<Stream0<DMA2>, 0, Adc<ADC1>, PeripheralToMemory, &'static mut [u16; 10]>;

    #[shared]
    struct Shared {
        serial: SerialPort<'static, UsbBusType>,
        usb_dev: UsbDevice<'static, UsbBusType>,
    }

    #[local]
    struct Local {
        sample_id: u32,
        p: Producer<'static, Sample, 64>,
        c: Consumer<'static, Sample, 64>,
        adc_transfer: DMATransfer,
        adc_buffer: Option<&'static mut [u16; 10]>,
        svm: IterativeSVM,
        u: PwmChannel<TIM1, 0_u8>,
        v: PwmChannel<TIM1, 1_u8>,
        w: PwmChannel<TIM1, 2_u8>,
    }

    #[init(local = [adc_buffer_: [u16; 10] = [0; 10],
    adc_buffer2: [u16; 10] = [0; 10],
    ep_memory: [u32; 1024] = [0; 1024],
    usb_bus: Option < UsbBusAllocator < UsbBus < USB >> > = None,
    q: Queue < Sample, 64 > = Queue::new()])]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        rtt_init_print!();

        rprintln!("init");
        let device: pac::Peripherals = cx.device;

        let rcc = device.RCC.constrain();
        let clocks = rcc
            .cfgr
            .use_hse(25.MHz())
            .sysclk(100.MHz())
            .hclk(100.MHz())
            .pclk1(50.MHz())
            .pclk2(100.MHz())
            .require_pll48clk()
            .freeze();
        rprintln!("hse");

        let mut timer = device.TIM2.counter(&clocks);

        unsafe {
            let tim = &(*TIM2::ptr());
            tim.ccmr1_output()
                .modify(|_, w| w.oc1pe().set_bit().oc1m().pwm_mode1());

            // Set the duty cycle
            tim.ccr1.modify(|_, w| w.ccr().bits(1));
            // Enable the channel
            tim.ccer.modify(|_, w| w.cc1e().set_bit());
            // Enable the TIM main Output
            tim.cr2.modify(|_, w| w.mms().bits(0b010));
        }

        let gpioa = device.GPIOA.split();
        let gpiob = device.GPIOB.split();

        let usb = USB {
            usb_global: device.OTG_FS_GLOBAL,
            usb_device: device.OTG_FS_DEVICE,
            usb_pwrclk: device.OTG_FS_PWRCLK,
            pin_dm: gpioa.pa11.into_alternate(),
            pin_dp: gpioa.pa12.into_alternate(),
            hclk: clocks.hclk(),
        };

        gpiob.pb2.into_floating_input();
        gpiob.pb3.into_floating_input();
        gpiob.pb4.into_floating_input();
        gpiob.pb5.into_floating_input();

        // leds
        gpiob.pb6.into_push_pull_output().set_high();
        gpiob.pb7.into_push_pull_output().set_high();
        gpiob.pb8.into_push_pull_output().set_high();
        gpiob.pb9.into_push_pull_output().set_high();

        // PWM enable
        gpiob.pb15.into_push_pull_output().set_high();

        let (mut ch_u, mut ch_v, mut ch_w) = device
            .TIM1
            .pwm_hz(
                (
                    gpioa.pa8.into_alternate(),
                    gpioa.pa9.into_alternate(),
                    gpioa.pa10.into_alternate(),
                ),
                200.kHz(),
                &clocks,
            )
            .split();

        // rprintln!("max duty: {}", max_duty);

        ch_u.set_duty(0);
        ch_v.set_duty(0);
        ch_w.set_duty(0);

        ch_u.enable();
        ch_v.enable();
        ch_w.enable();


        rprintln!("led on");

        let mut dcb = cx.core.DCB;
        let dwt = cx.core.DWT;
        let systick = cx.core.SYST;

        let mono = DwtSystick::new(&mut dcb, dwt, systick, MONO_HZ);

        cx.local
            .usb_bus
            .replace(UsbBusType::new(usb, cx.local.ep_memory));
        let serial = usbd_serial::SerialPort::new(cx.local.usb_bus.as_ref().unwrap());

        let mut usb_dev = UsbDeviceBuilder::new(
            cx.local.usb_bus.as_ref().unwrap(),
            UsbVidPid(0x16c0, 0x27dd),
        )
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(usbd_serial::USB_CLASS_CDC)
        .build();

        usb_dev.force_reset();
        rprintln!("usb dev start");

        let mut usb_pull = gpioa.pa15.into_push_pull_output();

        usb_pull.set_low();
        timer.start(10.millis()).unwrap();
        block!(timer.wait()).unwrap();
        usb_pull.set_high();

        timer
            .start(Duration::<u32, 1, 2_000_000>::from_ticks(250))
            .unwrap();
        timer.listen(Event::Update);

        let dma = StreamsTuple::new(device.DMA2);
        let config = DmaConfig::default()
            .transfer_complete_interrupt(true)
            .memory_increment(true)
            .double_buffer(false);

        let adc_config = AdcConfig::default()
            .external_trigger(TriggerMode::RisingEdge, ExternalTrigger::Tim_2_trgo)
            .dma(Dma::Continuous)
            .scan(Scan::Enabled)
            .clock(Clock::Pclk2_div_4);

        let mut adc = Adc::adc1(device.ADC1, true, adc_config);
        // vbias
        adc.configure_channel(
            &gpiob.pb0.into_analog(),
            Sequence::from(0),
            SampleTime::Cycles_15,
        );
        // encoder
        adc.configure_channel(
            &gpioa.pa1.into_analog(),
            Sequence::from(1),
            SampleTime::Cycles_15,
        );
        adc.configure_channel(
            &gpioa.pa0.into_analog(),
            Sequence::from(2),
            SampleTime::Cycles_15,
        );
        adc.configure_channel(
            &gpioa.pa3.into_analog(),
            Sequence::from(3),
            SampleTime::Cycles_15,
        );
        adc.configure_channel(
            &gpioa.pa2.into_analog(),
            Sequence::from(4),
            SampleTime::Cycles_15,
        );
        // current u, v, w
        adc.configure_channel(
            &gpioa.pa7.into_analog(),
            Sequence::from(5),
            SampleTime::Cycles_15,
        );
        adc.configure_channel(
            &gpioa.pa6.into_analog(),
            Sequence::from(6),
            SampleTime::Cycles_15,
        );
        adc.configure_channel(
            &gpioa.pa5.into_analog(),
            Sequence::from(7),
            SampleTime::Cycles_15,
        );

        // vbus
        adc.configure_channel(
            &gpioa.pa4.into_analog(),
            Sequence::from(8),
            SampleTime::Cycles_15,
        );

        // temp
        adc.configure_channel(&Temperature, Sequence::from(9), SampleTime::Cycles_15);

        adc.enable_temperature_and_vref();

        let mut adc_transfer =
            Transfer::init_peripheral_to_memory(dma.0, adc, cx.local.adc_buffer2, None, config);
        adc_transfer.start(|_| {});

        usb_idle_polling::spawn().ok().unwrap();
        rprintln!("spawned");

        let (p, c) = cx.local.q.split();

        (
            Shared { serial, usb_dev },
            Local {
                sample_id: 0,
                p,
                c,
                adc_buffer: Some(cx.local.adc_buffer_),
                adc_transfer,
                svm: IterativeSVM::new(40),
                u: ch_u,
                v: ch_v,
                w: ch_w,
            },
            init::Monotonics(mono),
        )
    }

    fn poll_usb(
        serial: &mut SerialPort<'static, UsbBusType>,
        usb_dev: &mut UsbDevice<'static, UsbBusType>,
        queue: Option<&mut Consumer<'static, Sample, 64>>,
    ) {
        if let Some(c) = queue {
            loop {
                match c.dequeue() {
                    Some(s) => {
                        let mut slice = [0; 100];
                        let length = bincode::encode_into_slice(
                            s,
                            &mut slice,
                            bincode::config::standard()
                                .with_little_endian()
                                .with_fixed_int_encoding()
                                .skip_fixed_array_length(),
                        )
                        .unwrap();

                        assert_eq!(length, 30);

                        let mut write_offset = 0;
                        while write_offset < length {
                            match serial.write(&slice[write_offset..length]) {
                                Ok(len) if len > 0 => {
                                    write_offset += len;
                                }
                                _ => {}
                            }
                            usb_dev.poll(&mut [serial]);
                        }
                    }
                    None => {
                        break;
                    }
                }
                usb_dev.poll(&mut [serial]);
            }
        }
    }

    // #[task(binds = OTG_FS, shared = [serial, usb_dev], priority = 2)]
    // fn usb_interrupt_polling(mut cx: usb_interrupt_polling::Context) {
    //     // (cx.shared.serial, cx.shared.usb_dev).lock(|serial: &mut SerialPort<_>,
    //     //                                             usb_dev: &mut UsbDevice<_>| {
    //     //     usb_dev.poll(&mut [serial]);
    //     // });
    // }

    #[task(shared = [serial, usb_dev], local = [c])]
    fn usb_idle_polling(cx: usb_idle_polling::Context) {
        let usb_idle_polling::Context { shared, local } = cx;

        (shared.serial, shared.usb_dev).lock(
            |serial: &mut SerialPort<_>, usb_dev: &mut UsbDevice<_>| {
                poll_usb(serial, usb_dev, Some(local.c))
            },
        );
        usb_idle_polling::spawn_after(500.micros()).ok();
    }

    #[task(binds = DMA2_STREAM0, local = [adc_buffer, p, sample_id, adc_transfer, svm, u, v, w], priority = 10)]
    fn dma(cx: dma::Context) {
        let dma::Context { local } = cx;
        let dma::LocalResources {
            adc_buffer,
            p,
            sample_id,
            adc_transfer,
            svm,
            u,
            v,
            w,
        } = local;
        let (buffer, _) = adc_transfer
            .next_transfer(adc_buffer.take().unwrap())
            .unwrap();

        let amplitude = 3.0;
        let frequency = 25000.0;

        let position = libm::sinf(*sample_id as f32 / frequency) * amplitude;
        let position = *sample_id as f32 / frequency;

        let phase = position * 6.2831853071f32;

        let (u_d, v_d, w_d) = svm.calculate(libm::sinf(phase), libm::cosf(phase));

        u.set_duty(u_d + 10);
        v.set_duty(v_d + 10);
        w.set_duty(w_d + 10);

        let vbus_pin = buffer[8] as f32 / 4096.0 * 3.3;
        let vbus = vbus_pin * 10.0;

        drop(p.enqueue(Sample {
            magic: 0xffff,
            id: (*sample_id % (1 << 15)) as u16,
            adc: *buffer,
            pwm: [u_d, v_d, w_d],
        }));

        *sample_id = sample_id.wrapping_add(1);
        *adc_buffer = Some(buffer);

        // rprintln!("u {} v {} w {}", u_d, v_d, w_d);
    }
}
