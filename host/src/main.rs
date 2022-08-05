mod comms;
mod selector;
mod channel_selector;
mod scope_interface;

use std::io::{BufReader, BufWriter, IoSlice, Read, Write};
use std::time::{Duration, Instant};

use core::hash::Hasher;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Arguments;
use std::iter::zip;
use std::sync::{Arc, mpsc};
use std::sync::mpsc::{channel, Sender, Receiver, RecvTimeoutError};
use std::thread::{Scope, spawn};
use remote_obj::prelude::*;

use rusb::{Context, Device, DeviceDescriptor, DeviceHandle, Direction, GlobalContext, open_device_with_vid_pid, Recipient, TransferType, UsbContext};
use common::{BINCODE_CFG, DeviceToHost, HostToDevice, ScopePacket, ContainerGetter, Container, ContainerValue, ContainerSetter};
use crate::comms::{ArbiterReq, CachedGetterSetter, new_device_pair, new_interface};

use eframe::{App, egui};
use egui::plot::{Line, Plot, Legend, PlotPoints};
use crate::channel_selector::ChannelSelector;
use crate::scope_interface::ScopeInterface;
use crate::selector::GetterSelector;

use std::{thread, time};
use std::fs::File;
use npyz::WriterBuilder;
use rand::seq::index::sample;

struct Plotter {
    lines: HashMap<ContainerGetter, VecDeque<(u32, f32)>>,
    lines_history: HashMap<ContainerGetter, VecDeque<(u32, f32)>>,

    scope: ScopeInterface,

    arb: Sender<ArbiterReq>,
    subsampling: u32,
    plot_time: f32,
    pos_setpoint: Option<CachedGetterSetter>,
    channel_selector: ChannelSelector,
    selected_channels: HashSet<ContainerGetter>,

    last_frame_time: Duration
}

impl Plotter {
    pub fn new(scope: ScopeInterface, arb: Sender<ArbiterReq>) -> Self {
        ArbiterReq::other(HostToDevice::ClearProbes, &arb);
        Plotter {
            lines: HashMap::new(),
            lines_history: HashMap::new(),
            scope,
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
            channel_selector: ChannelSelector::new(),
            selected_channels: HashSet::new(),
            last_frame_time: Duration::ZERO
        }
    }

    pub fn set_subsampling(&mut self, subsampling: u32) {
        self.subsampling = subsampling;
        ArbiterReq::other(HostToDevice::ProbeInterval(subsampling), &self.arb);
    }

    pub fn recv(&mut self) {
        let mut last_id = None;
        while let Ok((id, packet)) = self.scope.try_recv() {
            last_id = Some(id);

            for (&k, &v) in packet.iter() {
                let line = self.lines.entry(k).or_insert(VecDeque::new());
                line.push_back((id, v.as_float().unwrap()));
            }
        }

        // we got some packets this frame, truncate to the given number of seconds
        if let Some(last_id) = last_id {
            self.lines.iter_mut().map(|(k, v) | {
                let idx = v.partition_point(|&(id, _)| (id + (8000.0 * self.plot_time) as u32) < last_id);
                v.drain(..idx)
            }).for_each(drop);
        }
    }
}

impl eframe::App for Plotter {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let start = Instant::now();

        let last_frame_time = self.last_frame_time;
        egui::SidePanel::left("left panel").show(ctx, |mut ui| {
            if let Some(pos_setpoint) = self.pos_setpoint.as_mut() {
                let slider = egui::Slider::from_get_set(-100.0..=0.0, pos_setpoint.getter_setter()).text("Pos setpoint").smart_aim(false);
                ui.add(slider);
            }

            let selected_channels = self.channel_selector.ui(&mut ui);
            self.scope.req_set(selected_channels.clone());
            self.selected_channels = selected_channels;
            self.recv();
            ui.label(format!("frame processed in {:?}", last_frame_time));
        });

        egui::CentralPanel::default().show(ctx, |mut ui| {
            let mut plot = Plot::new("lines_demo").legend(Legend::default());

            let lines = self.lines.clone();
            let lines_history = self.lines_history.clone();

            plot.show(ui, |plot_ui| {

                let draw_lines;
                if plot_ui.is_auto_bounds().x {
                    draw_lines = lines;
                    self.lines_history = self.lines.clone()
                } else {
                    draw_lines = lines_history;
                }
                for (getter, values) in draw_lines {
                    if self.selected_channels.clone().contains(&getter) {
                        let name = format!("{}", getter);
                        let line = Line::new(PlotPoints::from_iter(
                            values.iter().map(|(id, value)| [*id as f64 / 8000.0, *value as f64])
                        )).name(name);
                        plot_ui.line(line);
                    }
                }
            });

            ctx.request_repaint();
        });

        self.last_frame_time = Instant::now() - start;
    }
}

impl Plotter {
    fn save_data(&mut self) {
        self.set_subsampling(1);
        self.plot_time = 1e3;

        let samples = 200_000;

        let channels = [
            getter!(Container.adc[0]),
            getter!(Container.adc[1]),
            getter!(Container.adc[2]),
            getter!(Container.adc[3]),
            getter!(Container.adc[4]),
            getter!(Container.adc[5]),
            getter!(Container.adc[6]),
            getter!(Container.adc[7]),
            getter!(Container.adc[8]),
        ];
        let ten_millis = time::Duration::from_millis(10);

        for c in channels {
            self.channel_selector.selectors.push(GetterSelector::from_getter(c));
            thread::sleep(ten_millis);
        }
        self.scope.req_set(self.channel_selector.get_selectors());

        loop {
            self.recv();

            if let Some(v) = self.lines.get(&channels[0]) {
                if v.len() > samples + 100 {
                    break
                }
                println!("{}", v.len())
            }

            thread::sleep(ten_millis);
        }

        for c in channels {
            println!("{:?} {}", c, self.lines[&c].len())
        }

        let mut buffer = File::create("data.npy").unwrap();

        let mut writer = {
            npyz::WriteOptions::new()
                .default_dtype()
                .shape(&[channels.len() as u64, samples as u64])
                .writer(buffer)
                .begin_nd().unwrap()
        };

        for c in channels {
            let v = &self.lines[&c];
            writer.extend(v.range((v.len() - samples)..).map(|(idx, value)| value)).unwrap();
        }
        writer.finish().unwrap();
    }
}

fn main() {
    let (arb, scope ) = new_interface();
    let scope_interface = ScopeInterface::new(arb.clone(), scope);

    let mut plotter = Plotter::new(scope_interface, arb);

    // plotter.save_data();

    plotter.set_subsampling(1);
    plotter.plot_time = 2.0;

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Linear Motor GUI",
        options,
        Box::new(|_cc| Box::new(plotter)),
    );
}
