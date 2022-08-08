use std::collections::HashSet;
use crate::egui::Ui;
use common::ContainerGetter;
use crate::selector::GetterSelector;

pub struct ChannelSelector {
    pub selectors: Vec<GetterSelector>,
}

impl ChannelSelector {
    pub fn new() -> ChannelSelector {
        ChannelSelector {
            selectors: Vec::new(),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) -> HashSet<ContainerGetter> {
        let mut remove = Vec::new();
        for (idx, i) in self.selectors.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(i);
                if ui.button("X").clicked() {
                    remove.push(idx)
                };
            });
        }

        // go in reverse so it doesn't mess up idx of other items
        for &i in remove.iter().rev() {
            self.selectors.remove(i);
        }

        if ui.button("+").clicked() {
            self.selectors.push(GetterSelector::new())
        }

        self.get_selectors()
    }

    pub fn get_selectors(&self) -> HashSet<ContainerGetter> {
        self.selectors.iter().filter_map(|x| x.getter).collect()
    }
}