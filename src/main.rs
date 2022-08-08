#![no_std]
#![no_main]
#![feature(proc_macro_hygiene)]

use panic_rtt_target as _;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [EXTI0, EXTI1])]
mod app {
    use dwt_systick_monotonic::DwtSystick;
    use dwt_systick_monotonic::fugit::Duration;

    use rtt_target::{self, rprintln, rtt_init_print};
    use stm32f4xx_hal::{
        adc::{
            Adc,
            config::{AdcConfig, Dma, SampleTime, Scan, Sequence, Clock, ExternalTrigger, TriggerMode},
        },
        dma::{config::DmaConfig, PeripheralToMemory, Stream0, StreamsTuple, Transfer},
        nb::block,
        otg_fs::{USB, UsbBus, UsbBusType},
        pac::{self, ADC1, DMA2, TIM1, TIM2},
        prelude::*,
        // signature::{VtempCal110, VtempCal30},
        timer::*,
        gpio::{Output, Pin},
    };

    use foc::state_machine::{PWMCommand, Controller};
    use config::Config;

    use encoder::EncoderState;

    use heapless::spsc::Queue;
    use bbqueue::BBBuffer;

    use usb_device::bus::UsbBusAllocator;
    use usb_device::prelude::*;
    use usbd_serial::CdcAcmClass;

    use common::*;

    mod comms;
    use comms::*;

    pub struct MotorOutputBlock {
        u: PwmChannel<TIM1, 0_u8>,
        v: PwmChannel<TIM1, 1_u8>,
        w: PwmChannel<TIM1, 2_u8>,
        pwm_en: Pin<'C', 6_u8, Output>,
    }

    impl MotorOutputBlock {
        fn set_duty(&mut self, pwm_req: &PWMCommand) {
            if pwm_req.driver_enable {
                self.u.set_duty(pwm_req.u_duty);
                self.v.set_duty(pwm_req.v_duty);
                self.w.set_duty(pwm_req.w_duty);
                self.pwm_en.set_high();
            } else {
                self.u.set_duty(0);
                self.v.set_duty(0);
                self.w.set_duty(0);
                self.pwm_en.set_low();
            }
        }
    }

    const MONO_HZ: u32 = 100_000_000;

    // 8 MHz
    #[monotonic(binds = SysTick, default = true)]
    type MyMono = DwtSystick<MONO_HZ>;

    type DMATransfer =
        Transfer<Stream0<DMA2>, 0, Adc<ADC1>, PeripheralToMemory, &'static mut [u16; 16]>;

    #[shared]
    struct Shared {
    }

    const SEND_BUF: usize = 8192;
    const RECV_BUF: usize = 32;

    #[local]
    struct Local {
        p: ControllerComms<SEND_BUF, RECV_BUF>,
        c: USBCommunicator<SEND_BUF, RECV_BUF>,
        adc_transfer: DMATransfer,
        adc_buffer: Option<&'static mut [u16; 16]>,
        controller: Controller,
        encoder: EncoderState,
        config: Config,
        pwm: MotorOutputBlock,
    }

    #[init(local = [adc_buffer_: [u16; 16] = [0; 16],
    adc_buffer2: [u16; 16] = [0; 16],
    ep_memory: [u32; 1024] = [0; 1024],
    usb_bus: Option < UsbBusAllocator < UsbBus < USB >> > = None,
    bbq: BBBuffer< SEND_BUF > = BBBuffer::new(),
    cmd_q: Queue< HostToDevice, RECV_BUF > = Queue::new()])]
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

        let mut ctrl_timer = device.TIM2.counter(&clocks);

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
        let gpioc = device.GPIOC.split();

        let usb = USB {
            usb_global: device.OTG_FS_GLOBAL,
            usb_device: device.OTG_FS_DEVICE,
            usb_pwrclk: device.OTG_FS_PWRCLK,
            pin_dm: gpioa.pa11.into_alternate(),
            pin_dp: gpioa.pa12.into_alternate(),
            hclk: clocks.hclk(),
        };

        // leds
        // gpiob.pb12.into_push_pull_output().set_high();
        // gpiob.pb13.into_push_pull_output().set_high();
        gpiob.pb14.into_push_pull_output().set_high();
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
        let serial = CdcAcmClass::new(cx.local.usb_bus.as_ref().unwrap(), 64);

        let mut usb_dev = UsbDeviceBuilder::new(
            cx.local.usb_bus.as_ref().unwrap(),
            UsbVidPid(0x1209, 0x0001),
        )
        .manufacturer("Fake company")
        .product("Serial port")
        .serial_number("TEST")
        .device_class(usbd_serial::USB_CLASS_CDC)
        .build();

        let _ = usb_dev.force_reset();
        rprintln!("usb dev start");

        let mut usb_pull = gpioa.pa15.into_push_pull_output();

        usb_pull.set_low();
        ctrl_timer.start(10.millis()).unwrap();
        block!(ctrl_timer.wait()).unwrap();
        usb_pull.set_high();

        ctrl_timer
            .start(Duration::<u32, 1, 2_000_000>::from_ticks(250))
            .unwrap();
        ctrl_timer.listen(Event::Update);

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
            &gpioa.pa5.into_analog(),
            Sequence::from(0),
            SampleTime::Cycles_480,
        );
        // encoder
        adc.configure_channel(
            &gpioa.pa6.into_analog(),
            Sequence::from(1),
            SampleTime::Cycles_28,
        );
        adc.configure_channel(
            &gpioa.pa4.into_analog(),
            Sequence::from(2),
            SampleTime::Cycles_28,
        );
        adc.configure_channel(
            &gpioa.pa3.into_analog(),
            Sequence::from(3),
            SampleTime::Cycles_28,
        );
        adc.configure_channel(
            &gpioc.pc2.into_analog(),
            Sequence::from(4),
            SampleTime::Cycles_28,
        );
        let pa1 = gpioa.pa0.into_analog();
        let pc3 = gpioa.pa2.into_analog();

        adc.configure_channel(
            &pa1,
            Sequence::from(5),
            SampleTime::Cycles_28,
        );
        adc.configure_channel(
            &pc3,
            Sequence::from(6),
            SampleTime::Cycles_28,
        );
        adc.configure_channel(
            &pa1,
            Sequence::from(7),
            SampleTime::Cycles_28,
        );
        adc.configure_channel(
            &pc3,
            Sequence::from(8),
            SampleTime::Cycles_28,
        );
        // iref
        adc.configure_channel(
            &gpioa.pa7.into_analog(),
            Sequence::from(9),
            SampleTime::Cycles_28,
        );
        // current u, v, w
        adc.configure_channel(
            &gpiob.pb1.into_analog(),
            Sequence::from(10),
            SampleTime::Cycles_28,
        );
        adc.configure_channel(
            &gpiob.pb0.into_analog(),
            Sequence::from(11),
            SampleTime::Cycles_28,
        );
        adc.configure_channel(
            &gpioc.pc5.into_analog(),
            Sequence::from(12),
            SampleTime::Cycles_28,
        );

        // vbus
        adc.configure_channel(
            &gpioc.pc0.into_analog(),
            Sequence::from(13),
            SampleTime::Cycles_28,
        );
        // motor temp
        adc.configure_channel(
            &gpioc.pc1.into_analog(),
            Sequence::from(14),
            SampleTime::Cycles_28,
        );
        // drive temp
        adc.configure_channel(
            &gpioc.pc4.into_analog(),
            Sequence::from(15),
            SampleTime::Cycles_28,
        );

        let mut adc_transfer =
            Transfer::init_peripheral_to_memory(dma.0, adc, cx.local.adc_buffer2, None, config);
        adc_transfer.start(|_| {});

        usb_idle_polling::spawn().ok().unwrap();
        rprintln!("spawned");

        let (p, c) = get_comms_pair(cx.local.bbq, cx.local.cmd_q, serial, usb_dev);

        let config = Config::new();
        let controller = Controller::new(&config);
        (
            Shared { },
            Local {
                p,
                c,
                adc_buffer: Some(cx.local.adc_buffer_),
                adc_transfer,
                config,
                controller,
                encoder: EncoderState::new(),
                pwm: MotorOutputBlock {
                    u: ch_u,
                    v: ch_v,
                    w: ch_w,
                    pwm_en: gpioc.pc6.into_push_pull_output()
                }
            },
            init::Monotonics(mono),
        )
    }

    #[task(local = [c], priority = 1)]
    fn usb_idle_polling(cx: usb_idle_polling::Context) {
        cx.local.c.run();
    }

    #[task(local = [p, controller, config, pwm, encoder], priority = 2, capacity = 1)]
    fn control_loop(cx: control_loop::Context, buffer: [u16; 16]) {
        let control_loop::Context { local } = cx;
        let control_loop::LocalResources {
            p,
            controller,
            config,
            encoder,
            pwm
        } = local;

        let position = encoder.update([
            buffer[1] as f32,
            buffer[2] as f32,
            buffer[3] as f32,
            buffer[4] as f32,
            buffer[5] as f32,
            buffer[6] as f32,
            buffer[7] as f32,
            buffer[8] as f32,
        ], &config);

        let update = to_controller_update(&buffer, position, &config);
        let mut pwm_req = controller.update(&update, &config);

        if update.phase_currents.max_magnitude() > config.hard_curr_limit {
            pwm_req.driver_enable = false;
        }

        if controller.encoder_ready() {
            encoder.calibration_done();
        }

        pwm.set_duty(&pwm_req);

        let mut container = Container {
            adc: &buffer,
            pwm: &pwm_req.to_array(),
            controller,
            update: &update,
            encoder,
            config,
        };

        p.tick(&mut container);
    }

    #[task(binds = DMA2_STREAM0, local = [adc_buffer, adc_transfer], priority = 5)]
    fn dma(cx: dma::Context) {
        let dma::Context { local } = cx;
        let dma::LocalResources {
            adc_buffer,
            adc_transfer,
        } = local;
        let (buffer, _) = adc_transfer
            .next_transfer(adc_buffer.take().unwrap())
            .unwrap();

        control_loop::spawn(buffer.clone()).unwrap();

        *adc_buffer = Some(buffer);

        // rprintln!("l");
    }
}
