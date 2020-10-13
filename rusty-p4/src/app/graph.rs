use crate::app::common::MergeResult;
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType, Link};
use petgraph::prelude::{EdgeRef, NodeIndex};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Formatter;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use petgraph::stable_graph::StableGraph;

#[derive(Debug)]
pub struct DefaultGraph {
    base: StableGraph<DeviceID, u32>,
    node_index: HashMap<DeviceID, petgraph::prelude::NodeIndex>,
    edge_to_link: HashMap<petgraph::prelude::EdgeIndex, Link>,
    link_to_edge: HashMap<Link, petgraph::prelude::EdgeIndex>,
}

impl DefaultGraph {
    pub fn new() -> DefaultGraph {
        DefaultGraph {
            base: Default::default(),
            node_index: HashMap::new(),
            edge_to_link: HashMap::new(),
            link_to_edge: HashMap::new(),
        }
    }

    pub fn add_device(&mut self, device: DeviceID) {
        if self.node_index.contains_key(&device) {
            return;
        }
        let node = self.base.add_node(device);
        self.node_index.insert(device, node);
    }

    pub fn remove_device(&mut self, device: &DeviceID) {
        if let Some(index) = self.node_index.remove(device) {
            self.base.remove_node(index);
        }
    }

    pub fn add_link(&mut self, link: &Link, cost: u32) -> MergeResult<()> {
        if let Some(index) = self.link_to_edge.get(link) {
            *self.base.edge_weight_mut(*index).unwrap() = cost;
            MergeResult::MERGED
        } else {
            let src = *self.node_index.get(&link.src.device).unwrap();
            let dst = *self.node_index.get(&link.dst.device).unwrap();
            let index = self.base.add_edge(src, dst, cost);
            self.link_to_edge.insert(*link, index);
            self.edge_to_link.insert(index, *link);
            MergeResult::ADDED(())
        }
    }

    pub fn remove_link(&mut self, link: &Link) {
        if let Some(index) = self.link_to_edge.remove(link) {
            self.edge_to_link.remove(&index);
            self.base.remove_edge(index);
        }
    }

    pub fn get_path(&self, src: DeviceID, dst: DeviceID) -> Option<Path> {
        let src = *self.node_index.get(&src).unwrap();
        let dst = *self.node_index.get(&dst).unwrap();

        let mut dist = HashMap::new();
        dist.insert(src, 0);
        let mut prev = HashMap::new();
        let mut visited = HashSet::with_capacity(self.base.node_count());

        let mut vertx_heap = BinaryHeap::with_capacity(self.base.node_count());
        vertx_heap.push(ReversePrioritywithVertx {
            vertx: src,
            priority: 0,
        });

        while let Some(ReversePrioritywithVertx { vertx, priority }) = vertx_heap.pop() {
            if !visited.insert(vertx) {
                continue;
            }

            for i in self.base.edges(vertx) {
                if *i.weight() != 0 {
                    let cost = *i.weight();
                    let new_dist = priority + cost;
                    let target = i.target();
                    let is_shorter = dist
                        .get(&target)
                        .map_or(true, |&current| new_dist < current);

                    if is_shorter {
                        dist.insert(target, new_dist);
                        prev.insert(target, i);
                        vertx_heap.push(ReversePrioritywithVertx {
                            vertx: target,
                            priority: new_dist,
                        })
                    }
                }
            }
        }
        if let Some(&p) = prev.get(&dst) {
            let mut lists = Vec::new();
            let mut one = p;
            let mut two = dst;
            let mut c = 0;
            loop {
                let link = self.edge_to_link.get(&p.id()).unwrap();
                lists.push(link.clone());
                c += p.weight();
                if one.source() == src {
                    break;
                }
                two = one.source();
                one = *prev.get(&two).unwrap();
            }
            lists.reverse();
            Some(Path {
                links: lists,
                weight: c,
            })
        } else {
            None
        }
    }

    pub fn get_weighted_path<T:Weigher>(&self, src:DeviceID,dst:DeviceID, weigher:&mut T) -> Option<Path> {
        let src = *self.node_index.get(&src).unwrap();
        let dst = *self.node_index.get(&dst).unwrap();

        let mut dist = HashMap::new();
        dist.insert(src, 0);
        let mut prev = HashMap::new();
        let mut visited = HashSet::with_capacity(self.base.node_count());

        let mut vertx_heap = BinaryHeap::with_capacity(self.base.node_count());
        vertx_heap.push(ReversePrioritywithVertx {
            vertx: src,
            priority: 0,
        });

        while let Some(ReversePrioritywithVertx { vertx, priority }) = vertx_heap.pop() {
            if !visited.insert(vertx) {
                continue;
            }

            for i in self.base.edges(vertx) {
                let link = i.id();
                let w = self.edge_to_link.get(&link).and_then(|l|{
                    weigher.get(l)
                }).unwrap_or(weigher.default());
                if w != 0 {
                    let cost = w;
                    let new_dist = priority + cost;
                    let target = i.target();
                    let is_shorter = dist
                        .get(&target)
                        .map_or(true, |&current| new_dist < current);

                    if is_shorter {
                        dist.insert(target, new_dist);
                        prev.insert(target, i);
                        vertx_heap.push(ReversePrioritywithVertx {
                            vertx: target,
                            priority: new_dist,
                        })
                    }
                }
            }
        }
        if let Some(&p) = prev.get(&dst) {
            let mut lists = Vec::new();
            let mut one = p;
            let mut two = dst;
            let mut c = 0;
            loop {
                let link = self.edge_to_link.get(&p.id()).unwrap();
                lists.push(link.clone());
                c += p.weight();
                if one.source() == src {
                    break;
                }
                two = one.source();
                one = *prev.get(&two).unwrap();
            }
            lists.reverse();
            Some(Path {
                links: lists,
                weight: c,
            })
        } else {
            None
        }
    }

    pub fn weighted_map<T:Weigher>(&self, weigher:&mut T) -> DefaultGraph {
        let mut new_graph = self.base.clone();
        for (link,edge) in self.link_to_edge.iter() {
            if let Some(w) = new_graph.edge_weight_mut(*edge) {
                *w = weigher.get(link).unwrap_or(weigher.default());
            }
            else {
                unreachable!()
            }
        }
        DefaultGraph {
            base: new_graph,
            node_index: self.node_index.clone(),
            edge_to_link: self.edge_to_link.clone(),
            link_to_edge: self.link_to_edge.clone()
        }
    }
}

pub trait Weigher {
    fn get(&mut self,link:&Link) -> Option<u32>;
    fn default(&mut self) -> u32;
}

#[derive(Debug)]
pub struct Path {
    pub links: Vec<Link>,
    pub weight: u32,
}

#[derive(Eq)]
struct ReversePrioritywithVertx {
    pub vertx: NodeIndex,
    pub priority: u32,
}

impl Ord for ReversePrioritywithVertx {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority.cmp(&self.priority)
    }
}

impl PartialOrd for ReversePrioritywithVertx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ReversePrioritywithVertx {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}
