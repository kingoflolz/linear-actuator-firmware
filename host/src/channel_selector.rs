use std::collections::HashSet;
use egui::{Response, Ui, Widget};
use remote_obj::prelude::*;
use common::{Container, ContainerGetter, HostToDevice};
use rand::Rng;
use crate::selector::GetterSelector;
use std::sync::mpsc::Sender;
use crate::ArbiterReq;

pub struct ChannelSelector {
    selectors: Vec<GetterSelector>,
    pub(crate) getters: Vec<ContainerGetter>,
}

impl ChannelSelector {
    pub fn new() -> ChannelSelector {
        ChannelSelector {
            selectors: Vec::new(),
            getters: Vec::new(),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) -> HashSet<ContainerGetter> {
        let mut remove = Vec::new();
        for (idx, mut i) in self.selectors.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(i);
                if ui.button("x").clicked() {
                    remove.push(idx)
                };
            });
        }

        for &i in remove.iter().rev() {
            self.selectors.remove(i);
        }

        if ui.button("+").clicked() {
            self.selectors.push(GetterSelector::new())
        }

        self.selectors.iter().filter_map(|x| x.getter).collect()
    }
}