use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;
use crate::egui::{Response, Ui, Widget};
use remote_obj::prelude::*;
use common::{Container, ContainerGetter, ContainerValue};
use rand::Rng;
use eventuals::*;
use crate::ArbiterReq;

struct VariableData {
    eventual: Eventual<ContainerValue>,
    writer: EventualWriter<ContainerValue>,

    pending_req: Option<Result<ContainerValue, ()>>,

    last_requested_from_device: Option<Instant>,
    last_requested_from_ui: Instant,
}

impl VariableData {
    fn new() -> Self {
        let (writer, eventual) = Eventual::new();
        VariableData {
            eventual,
            writer,
            pending_req: None,
            last_requested_from_device: None,
            last_requested_from_ui: Instant::now(),
        }
    }
}

pub struct VariableGetter {
    variables: HashMap<ContainerGetter, VariableData>,
    arb: Sender<ArbiterReq>,
}

impl VariableGetter {
    pub fn new(arb: Sender<ArbiterReq>) -> Self {
        VariableGetter {
            variables: HashMap::new(),
            arb: arb.clone(),
        }
    }

    pub fn get(&mut self, getter: ContainerGetter) -> Eventual<ContainerValue> {
        let mut entry = self.variables.entry(getter).or_insert_with(|| {
            VariableData::new()
        });
        entry.last_requested_from_ui = Instant::now();
        entry.eventual.clone()
    }
}