use std::collections::{HashMap, HashSet, VecDeque};
use std::iter::zip;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::Duration;
use common::{Container, DeviceToHost, HostToDevice};
use crate::ArbiterReq;
use common::{ContainerGetter, ContainerValue};

pub struct ScopeInterface {
    recv: Receiver<DeviceToHost>,
    arb: Sender<ArbiterReq>,

    current_requests: HashSet<ContainerGetter>,

    /// this *should* stay in perfect sync with the probes on the remote
    synced_probes: Vec<ContainerGetter>,

    /// this contains messages which have been sent to the device already, but have not been applied
    /// applied to synced_probes yet (as we have not received confirmation yet)
    pending: VecDeque<HostToDevice>,

    last_id: Option<u32>
}

impl ScopeInterface {
    pub fn new(arb: Sender<ArbiterReq>, recv: Receiver<DeviceToHost>) -> Self {
        let timeout = Duration::from_millis(1000);
        arb.send(ArbiterReq::Other(HostToDevice::ClearProbes)).unwrap();

        // wait until we get confirmation that its cleared
        loop {
            let msg = recv.recv_timeout(timeout).unwrap();
            match msg {
                DeviceToHost::ProbeCleared => {
                    break
                }
                _ => {}
            }
        }

        ScopeInterface {
            recv,
            arb,
            current_requests: HashSet::new(),
            synced_probes: Vec::new(),
            pending: VecDeque::from([HostToDevice::ClearProbes]),
            last_id: None
        }
    }

    pub fn req_set(&mut self, new_requests: HashSet<ContainerGetter>) {
        for addition in new_requests.difference(&self.current_requests.clone()) {
            let add = HostToDevice::AddProbe(*addition);
            self.arb.send(ArbiterReq::Other(add.clone())).unwrap();
            self.pending.push_back(add.clone());
            self.current_requests.insert(*addition);
        }

        for deletion in self.current_requests.clone().difference(&new_requests) {
            // its possible we've added something and its in `pending` but not in synced_probes,
            // so we will defer its removal until its added to synced_probes

            let index = self.synced_probes.iter().position(|x| x == deletion);
            if let Some(index) = index {
                let remove = HostToDevice::RemoveProbe(index as u8);
                self.arb.send(ArbiterReq::Other(remove.clone())).unwrap();
                self.pending.push_back(remove.clone());
                self.current_requests.remove(deletion);
            }
        }
    }

    pub fn try_recv(&mut self) -> Result<(u32, HashMap<ContainerGetter, ContainerValue>), TryRecvError> {
        let message = self.recv.try_recv()?;

        match message {
            DeviceToHost::Sample(packet) => {
                let mut ret = HashMap::new();
                let results = packet.rehydrate(&self.synced_probes);
                for (r, g) in zip(results.iter(), &self.synced_probes) {
                    if let Some(result) = r {
                        ret.insert(*g, *result);
                    }
                }
                if let Some(last_id) = self.last_id {
                    if last_id.wrapping_add(1) != packet.id {
                        println!("last {} current {}", last_id, packet.id);
                    }
                }
                return Ok((packet.id, ret))
            }
            DeviceToHost::ProbeAdded => {
                let req = self.pending.pop_front().unwrap();

                match req {
                    HostToDevice::AddProbe(g) => {
                        self.synced_probes.push(g)
                    }
                    _ => unreachable!()
                }
            }
            DeviceToHost::ProbeRemoved => {
                let req = self.pending.pop_front().unwrap();

                match req {
                    HostToDevice::RemoveProbe(idx) => {
                        self.synced_probes.swap_remove(idx as usize);
                    }
                    _ => unreachable!()
                }
            }
            DeviceToHost::ProbeCleared => {
                let req = self.pending.pop_front().unwrap();

                match req {
                    HostToDevice::ClearProbes => {
                        self.synced_probes.clear()
                    }
                    _ => unreachable!()
                }
            }
            _ => unreachable!()
        }

        self.try_recv()
    }
}