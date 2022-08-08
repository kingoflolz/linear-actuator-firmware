use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;
use remote_obj::prelude::*;
use common::{Container, ContainerGetter, ContainerValue};
use eventuals::*;
use crate::ArbiterReq;
use crate::comms::GetterSetter;

struct VariableData {
    eventual: Eventual<ContainerValue>,
    writer: EventualWriter<ContainerValue>,

    pending_req: Option<Receiver<Result<ContainerValue, ()>>>,

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

    pub fn get_getter_setter(&mut self, getter: ContainerGetter) -> GetterSetter {
        GetterSetter::new(
            getter,
            Box::new(move |x| {
                Container::dynamic_setter_numeric(&getter.to_string(), x as f64).unwrap()
            }),
            self.get(getter),
            self.arb.clone()
        )
    }

    pub fn update(&mut self) {
        let mut i = 0;
        for (g, v) in self.variables.iter_mut() {
            if let Some(r) = v.pending_req.as_ref() {
                if let Ok(Ok(val)) = r.try_recv() {
                    v.writer.write(val);
                    v.pending_req = None;
                }

                if v.last_requested_from_device.unwrap().elapsed().as_secs_f32() > 0.2 {
                    v.pending_req = None;
                }
            } else if v.last_requested_from_ui.elapsed().as_secs_f32() < 0.2 && i < 10 {
                if v.last_requested_from_device.is_none() || v.last_requested_from_device.unwrap().elapsed().as_secs_f32() > 0.2  {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let req = ArbiterReq::Getter(g.clone(), tx);
                    self.arb.send(req).unwrap();
                    v.pending_req = Some(rx);
                    v.last_requested_from_device = Some(Instant::now());
                    i += 1;
                }
            }
        }
    }
}