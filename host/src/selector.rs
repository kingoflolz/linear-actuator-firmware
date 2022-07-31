use crate::egui::{Response, Ui, Widget};
use remote_obj::prelude::*;
use common::{Container, ContainerGetter};
use rand::Rng;

pub struct GetterSelector {
    pub indexes: Vec<usize>,
    pub getter: Option<ContainerGetter>,
    pub id: usize
}

impl GetterSelector {
    pub(crate) fn new() -> Self {
        let mut rng = rand::thread_rng();

        GetterSelector {
            indexes: vec![0],
            getter: None,
            id: rng.gen()
        }
    }
}

impl Widget for &mut GetterSelector {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            let mut getter_string = String::new();

            let idx_max = self.indexes.len() - 1;

            let mut add_new = false;
            let mut trunc_len = usize::MAX;
            for (idx, mut i) in self.indexes.iter_mut().enumerate() {
                let fields = ContainerGetter::get_fields(&getter_string).unwrap();

                let selections: Vec<String>;
                match fields {
                    FieldsType::Arr(max_len) => {
                        selections = [String::new()].into_iter().chain(
                            (0..max_len).into_iter().map(|x| {
                                format!("[{}]", x)
                            })
                        ).collect();
                    }
                    FieldsType::Fields(fields) => {
                        selections = [String::new()].into_iter().chain(
                            fields.into_iter().filter(|&x| {
                                *x != "VARIANT"
                            }).map(|x| {
                                x.to_string()
                            })
                        ).collect();
                    }
                    FieldsType::Terminal => {
                        trunc_len = idx + 1;
                        self.getter = Container::dynamic_getter(&getter_string);
                        break
                    }
                }

                let mut combo = crate::egui::ComboBox::from_id_source([self.id, idx]);
                if *i != 0 {
                    combo = combo.width(1.0)
                }

                let response = combo.show_index(
                    ui,
                    &mut i,
                    selections.len(),
                    |i| selections[i].to_string()
                );

                if *i != 0 {
                    getter_string.push_str(&selections[*i])
                }

                if idx == idx_max && *i != 0 {
                    add_new = true
                }

                if response.changed() {
                    trunc_len = idx + 1;
                    self.getter = None;
                    break
                }
            }
            self.indexes.truncate(trunc_len);

            if add_new {
                self.indexes.push(0);
            }
         }).response
    }
}