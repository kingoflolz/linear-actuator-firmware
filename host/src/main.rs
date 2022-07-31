mod comms;
mod selector;
mod channel_selector;

use std::io::{BufReader, BufWriter, IoSlice, Read, Write};
use std::time::Duration;
use egui::remap_clamp;

use core::hash::Hasher;
use std::collections::{HashMap, VecDeque};
use std::fmt::Arguments;
use std::iter::zip;
use std::sync::{Arc, mpsc};
use std::sync::mpsc::{channel, Sender, Receiver, RecvTimeoutError};
use std::thread::spawn;
use remote_obj::prelude::*;

use rusb::{Context, Device, DeviceDescriptor, DeviceHandle, Direction, GlobalContext, open_device_with_vid_pid, Recipient, Result, TransferType, UsbContext};
use common::{BINCODE_CFG, DeviceToHost, HostToDevice, ScopePacket, ContainerGetter, Container, ContainerValue, ContainerSetter};
use crate::comms::{ArbiterReq, CachedGetterSetter, new_device_pair, new_interface};

use eframe::egui;
use egui::plot::{Line, Plot, Values, Value as PlotValue, Legend};
use crate::channel_selector::ChannelSelector;

struct Plotter {
    lines: HashMap<ContainerGetter, VecDeque<(usize, f32)>>,
    receiver: Receiver<ScopePacket>,
    arb: Sender<ArbiterReq>,
    subsampling: u32,
    plot_time: f32,
    pos_setpoint: CachedGetterSetter,
    channel_selector: ChannelSelector
}

impl Plotter {
    pub fn new(receiver: Receiver<ScopePacket>, arb: Sender<ArbiterReq>) -> Self {
        ArbiterReq::other(HostToDevice::ClearProbes, &arb);
        Plotter {
            lines: HashMap::new(),
            receiver,
            arb: arb.clone(),
            subsampling: 1,
            plot_time: 2.0,
            pos_setpoint: {
                CachedGetterSetter::new(
                    getter!(Container.controller.voltage_controller::Foc.pos_controller.pos_setpoint),
                    Box::new(|v| {
                        setter!(Container.controller.voltage_controller::Foc.pos_controller.pos_setpoint = v)
                    }),
                    arb.clone()
                )
            },
            channel_selector: ChannelSelector::new(arb.clone())
        }
    }

    pub fn set_subsampling(&mut self, subsampling: u32) {
        self.subsampling = subsampling;
        ArbiterReq::other(HostToDevice::ProbeInterval(subsampling), &self.arb);
    }

    pub fn insert_packet(&mut self, packet: ScopePacket) {
        let results = packet.rehydrate(&self.channel_selector.getters);

        for (r, v) in zip(results.iter(), self.channel_selector.getters.iter()) {
            if !self.lines.contains_key(v) {
                self.lines.insert(v.clone(), VecDeque::new());
            }
            if let Some(result) = r {
                self.lines.get_mut(v).unwrap().push_front((
                    packet.id as usize,
                    result.as_float().unwrap()
                ));
            }
            self.lines.get_mut(v).unwrap().truncate((self.plot_time * 8000.0 / self.subsampling as f32) as usize);
        }
    }
}

impl eframe::App for Plotter {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |mut ui| {
            let mut plot = Plot::new("lines_demo").legend(Legend::default());

            let slider = egui::Slider::from_get_set(-100.0..=0.0, self.pos_setpoint.getter_setter()).text("Pos setpoint").smart_aim(false);
            ui.add(slider);

            self.channel_selector.ui(&mut ui);

            let r = plot.show(ui, |plot_ui| {
                for (getter, values) in &self.lines {
                    let name = format!("{}", getter);
                    let line = Line::new(Values::from_values_iter(
                        values.iter().map(|(id, value)| PlotValue::new(*id as f64 / 8000.0, *value as f64))
                    )).name(name);
                    plot_ui.line(line);
                }
            });

            while let Ok(packet) = self.receiver.try_recv() {
                self.insert_packet(packet);
            }

            ctx.request_repaint();
        });
    }
}

fn main() {
    let (arb, scope ) = new_interface();

    // cmd_s.send(HostToDevice::AddProbe(getter!(Container.adc[1]))).unwrap();
    // cmd_s.send(HostToDevice::AddProbe(getter!(Container.adc[2]))).unwrap();
    // cmd_s.send(HostToDevice::AddProbe(getter!(Container.adc[3]))).unwrap();

    let mut plotter = Plotter::new(scope, arb);
    plotter.set_subsampling(16);
    plotter.plot_time = 5.0;

    // plotter.add_probe(getter!(Container.encoder::Running.normalized[0]));
    // plotter.add_probe(getter!(Container.encoder::Running.normalized[1]));
    // plotter.add_probe(getter!(Container.encoder::Running.normalized[2]));
    // plotter.add_probe(getter!(Container.encoder::Running.normalized[3]));
    // plotter.add_probe(getter!(Container.update.phase_currents.u));
    // plotter.add_probe(getter!(Container.update.phase_currents.v));
    // plotter.add_probe(getter!(Container.update.phase_currents.w));
    //
    // plotter.add_probe(getter!(Container.pwm[0]));
    // plotter.add_probe(getter!(Container.pwm[1]));
    // plotter.add_probe(getter!(Container.pwm[2]));

    // plotter.add_probe(getter!(Container.controller.voltage_controller::Foc.dq_currents.d));
    // plotter.add_probe(getter!(Container.controller.voltage_controller::Foc.dq_currents.q));
    // plotter.add_probe(getter!(Container.controller.voltage_controller::Foc.q_req));
    // plotter.add_probe(getter!(Container.controller.voltage_controller::Foc.pos_controller.pos_setpoint));
    // plotter.add_probe(getter!(Container.controller.voltage_controller::Foc.encoder_output.position));
    // plotter.add_probe(getter!(Container.controller.voltage_controller::Foc.encoder_output.velocity));

    // plotter.add_probe(getter!(Container.adc[1]));
    // plotter.add_probe(getter!(Container.adc[2]));
    // plotter.add_probe(getter!(Container.adc[3]));
    // plotter.add_probe(getter!(Container.adc[4]));
    // plotter.add_probe(getter!(Container.adc[5]));
    // plotter.add_probe(getter!(Container.adc[6]));
    // plotter.add_probe(getter!(Container.adc[7]));
    // plotter.add_probe(getter!(Container.adc[8]));

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Linear Motor GUI",
        options,
        Box::new(|_cc| Box::new(plotter)),
    );
}
