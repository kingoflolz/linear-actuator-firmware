mod comms;
mod selector;
mod channel_selector;
mod scope_interface;
mod variable_getter;

use std::time::{Duration, Instant};

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::Sender;
use remote_obj::prelude::*;

use common::{HostToDevice, ContainerGetter, ContainerSetter, Container};
use crate::comms::{ArbiterReq, GetterSetter, new_interface};

use eframe::egui;
use crate::egui::{CollapsingHeader, Ui};

use egui::plot::{Line, Plot, Legend, PlotPoints};
use crate::channel_selector::ChannelSelector;
use crate::scope_interface::ScopeInterface;
use crate::selector::GetterSelector;
use crate::variable_getter::VariableGetter;

use std::{thread, time};
use std::fs::File;
use npyz::WriterBuilder;

struct GUI {
    lines: HashMap<ContainerGetter, VecDeque<(u32, f32)>>,
    lines_history: HashMap<ContainerGetter, VecDeque<(u32, f32)>>,

    scope: ScopeInterface,

    arb: Sender<ArbiterReq>,

    variable_getter: VariableGetter,

    subsampling: u32,
    plot_time: f64,
    pos_setpoint: GetterSetter,
    channel_selector: ChannelSelector,
    selected_channels: HashSet<ContainerGetter>,

    last_frame_time: Duration,
    last_frame_auto_bounds: bool
}

impl GUI {
    pub fn new(scope: ScopeInterface, arb: Sender<ArbiterReq>) -> Self {
        ArbiterReq::other(HostToDevice::ClearProbes, &arb);
        let mut variable_getter = VariableGetter::new(arb.clone());
        GUI {
            lines: HashMap::new(),
            lines_history: HashMap::new(),
            scope,
            arb: arb.clone(),
            subsampling: 1,
            plot_time: 2.0,
            pos_setpoint: variable_getter.get_getter_setter(getter!(Container.controller.voltage_controller::Foc.pos_controller.pos_setpoint)),
            variable_getter,
            channel_selector: ChannelSelector::new(),
            selected_channels: HashSet::new(),
            last_frame_time: Duration::ZERO,
            last_frame_auto_bounds: false
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
            self.lines.iter_mut().map(|(_, v) | {
                let idx = v.partition_point(|&(id, _)| (id + (8000.0 * self.plot_time) as u32) < last_id);
                v.drain(..idx)
            }).for_each(drop);
        }
    }
}

impl eframe::App for GUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let start = Instant::now();

        let last_frame_time = self.last_frame_time;
        egui::SidePanel::left("left panel").show(ctx, |mut ui| {
            ui.add(egui::Slider::new(&mut self.plot_time, 0.0..=60.0).text("max scope time"));

            if let Some(pos_setpoint) = self.pos_setpoint.getter_setter() {
                let slider = egui::Slider::from_get_set(-100.0..=0.0, pos_setpoint).text("Pos setpoint").smart_aim(false);
                ui.add(slider);
            }

            let selected_channels = self.channel_selector.ui(&mut ui);
            self.scope.req_set(selected_channels.clone());
            self.selected_channels = selected_channels;
            self.recv();
            ui.label(format!("frame processed in {:?}", last_frame_time));
        });

        fn draw_panel(ui: &mut Ui, getter_str: &str, new_section: &str, scope_selectors: &mut Vec<GetterSelector>, variable_getter: &mut VariableGetter) {
            let fields = ContainerGetter::get_fields(&getter_str);

            match fields {
                Some(FieldsType::Arr(max_len)) => {
                    CollapsingHeader::new(new_section).default_open(false).show(ui, |ui| {
                        (0..max_len).into_iter().map(|x| {
                            format!("[{}]", x)
                        }).for_each(|x| {
                            draw_panel(ui, &format!("{}{}", getter_str, x), &x, scope_selectors, variable_getter);
                        });
                    });
                }
                Some(FieldsType::Fields(fields)) => {
                    let mut draw_fields = |mut ui: &mut Ui| {
                        fields.into_iter().filter(|&x| {
                            *x != "VARIANT"
                        }).map(|x| {
                            x.to_string()
                        }).for_each(|x| {
                            draw_panel(&mut ui, &format!("{}{}", getter_str, x), &x, scope_selectors, variable_getter);
                        });
                    };

                    if new_section == "" {
                        draw_fields(ui);
                    } else {
                        CollapsingHeader::new(new_section).default_open(true).show(ui, |ui| {
                            draw_fields(ui);
                        });
                    }
                }
                Some(FieldsType::Terminal) => {
                    let getter = Container::dynamic_getter(&getter_str).unwrap();

                    ui.horizontal(|ui| {
                        let mut gs = variable_getter.get_getter_setter(getter);
                        let gs = gs.getter_setter();
                        ui.label(format!("{}", new_section));

                        if let Some(mut gs) = gs {
                            ui.add(egui::DragValue::from_get_set(&mut gs)
                                .max_decimals(6)
                                .speed(0.0)
                                .clamp_range(f64::NEG_INFINITY..=f64::INFINITY));
                        }

                        let idx = scope_selectors.iter().position(|x| if let Some(g) = x.getter {
                            g.to_string() == getter_str
                        } else {
                            false
                        });

                        if ui.button(if idx.is_none() {"📈"} else {"X"}).clicked() {
                            if let Some(idx) = idx {
                                scope_selectors.remove(idx);
                            } else {
                                scope_selectors.push(GetterSelector::from_getter(
                                    Container::dynamic_getter(&getter_str).unwrap()
                                ))
                            }
                        }
                    });
                }
                _ => {
                    ui.label(format!("failed to create getter!"));
                }
            }
        }

        egui::SidePanel::right("right panel").show(ctx, |mut ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                draw_panel(ui, "", "", &mut self.channel_selector.selectors, &mut self.variable_getter);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let plot = Plot::new("lines_demo").legend(Legend::default());

            let lines = self.lines.clone();

            plot.show(ui, |plot_ui| {

                let auto_bounds = plot_ui.is_auto_bounds().x;
                if self.last_frame_auto_bounds && !auto_bounds {
                    self.lines_history = self.lines.clone()
                }
                let lines_history = self.lines_history.clone();

                let draw_lines;
                if auto_bounds {
                    draw_lines = lines;
                } else {
                    draw_lines = lines_history;
                }

                self.last_frame_auto_bounds = plot_ui.is_auto_bounds().x;
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

        self.variable_getter.update();

        self.last_frame_time = Instant::now() - start;
    }
}

impl GUI {
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

        let buffer = File::create("data.npy").unwrap();

        let mut writer = {
            npyz::WriteOptions::new()
                .default_dtype()
                .shape(&[channels.len() as u64, samples as u64])
                .writer(buffer)
                .begin_nd().unwrap()
        };

        for c in channels {
            let v = &self.lines[&c];
            writer.extend(v.range((v.len() - samples)..).map(|(_, value)| value)).unwrap();
        }
        writer.finish().unwrap();
    }
}

fn main() {
    let (arb, scope ) = new_interface();
    let scope_interface = ScopeInterface::new(arb.clone(), scope);

    let mut plotter = GUI::new(scope_interface, arb);
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
