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
    getter_set: HashSet<ContainerGetter>,
    arb: Sender<ArbiterReq>
}

impl ChannelSelector {
    pub fn new(arb: Sender<ArbiterReq>) -> ChannelSelector {
        ChannelSelector {
            selectors: Vec::new(),
            getters: Vec::new(),
            getter_set: HashSet::new(),
            arb
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        for mut i in self.selectors.iter_mut() {
            ui.add(i);
        }
        if ui.button("+").clicked() {
            self.selectors.push(GetterSelector::new())
        }

        let new_getters_set: HashSet<_> = self.selectors.iter().filter_map(|x| x.getter).collect();
        for additions in new_getters_set.difference(&self.getter_set) {
            self.arb.send(ArbiterReq::Other(HostToDevice::AddProbe(*additions))).unwrap();
            self.getters.push(*additions)
        }

        for deletions in self.getter_set.difference(&new_getters_set) {
            unreachable!()
        }

        self.getter_set = new_getters_set;
    }
}