use crate::app::common::MergeResult;
use crate::representation::{ConnectPoint, Device, DeviceType, Link};
use bitfield::fmt::{Debug, Display};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Formatter;
use std::hash::Hash;

// index = x+y*n
pub struct GraphBase<T> {
    pub base: Vec<T>,
    n_capacity: usize,
    n_usage: usize,
}

impl<T> GraphBase<T>
where
    T: Default + Copy + Display,
{
    pub fn with_capacity(n: usize) -> GraphBase<T> {
        GraphBase {
            base: vec![T::default(); n * n],
            n_capacity: n,
            n_usage: 0,
        }
    }

    pub fn with_capacity_and_usage(cap: usize, usage: usize) -> GraphBase<T> {
        assert!(usage <= cap);
        GraphBase {
            base: vec![T::default(); cap * cap],
            n_capacity: cap,
            n_usage: usage,
        }
    }

    pub fn get(&self, x: usize, y: usize) -> &T {
        assert!(x < self.n_usage);
        assert!(y < self.n_usage);
        self.base.get(x + y * self.n_usage).unwrap()
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut T {
        assert!(x < self.n_usage);
        assert!(y < self.n_usage);
        self.base.get_mut(x + y * self.n_usage).unwrap()
    }

    pub fn set(&mut self, x: usize, y: usize, item: T) -> Result<(), ()> {
        assert!(x < self.n_usage);
        assert!(y < self.n_usage);
        assert!(x + y * self.n_usage < self.n_capacity * self.n_capacity);
        self.base[x + y * self.n_usage] = item;

        Ok(())
    }

    fn reserve(&mut self, n: usize) {
        let new_cap = self.n_capacity + n;
        let addition = new_cap * new_cap - self.n_capacity * self.n_capacity;
        self.base.reserve(addition);
        self.n_capacity = new_cap;
    }

    pub fn incr_space(&mut self, n: usize) {
        assert!(n + self.n_usage <= self.n_capacity);
        let new_n_usage = self.n_usage + n;
        if self.n_usage == 0 {
            self.n_usage = new_n_usage;
            return;
        }
        for y in (0..self.n_usage).rev() {
            for x in (0..self.n_usage).rev() {
                let old_index = x + y * self.n_usage;
                let new_index = old_index + y * n;
                self.base[new_index] = self.base[old_index];
            }
        }
        self.n_usage = new_n_usage;
    }

    pub fn map_from<A, F>(other: &GraphBase<A>, map: F) -> GraphBase<T>
    where
        F: Fn(&A) -> T,
    {
        let n = other.n_usage;
        let mut new_vec = Vec::with_capacity(other.n_capacity);
        for i in 0..n * n {
            new_vec.push(map(other.base.get(i).unwrap()));
        }
        GraphBase {
            base: new_vec,
            n_capacity: other.n_capacity,
            n_usage: other.n_usage,
        }
    }
}

impl<T> Debug for GraphBase<T>
where
    T: Display + Default + Copy,
{
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        writeln!(f, "n_usage: {}, n_cap: {}", self.n_usage, self.n_capacity);
        for x in 0..self.n_usage {
            for y in 0..self.n_usage {
                write!(f, "{} ", self.get(x, y));
            }
            writeln!(f);
        }
        Ok(())
    }
}

struct ShortestPathBuffer {
    base: GraphBase<u8>,
    dirty: bool,
}

#[derive(Debug)]
pub struct DefaultGraph {
    connectivity: GraphBase<u8>,
    index_map: HashMap<String, usize>,
    link_map: HashMap<(usize, usize), (Link, u8)>,
    node_count: usize,
}

impl DefaultGraph {
    pub fn new() -> DefaultGraph {
        DefaultGraph {
            connectivity: GraphBase::with_capacity_and_usage(10, 5),
            index_map: HashMap::new(),
            link_map: HashMap::new(),
            node_count: 0,
        }
    }

    pub fn add_device(&mut self, device: &Device) {
        if self.index_map.contains_key(&device.name) {
            return;
        }
        self.index_map.insert(device.name.clone(), self.node_count);
        self.node_count += 1;
        if self.node_count > self.connectivity.n_usage {
            self.connectivity
                .incr_space(self.node_count - self.connectivity.n_usage);
        }
    }

    pub fn add_link(&mut self, link: &Link, cost: u8) -> MergeResult<()> {
        let one = *self.index_map.get(link.src.device.as_str()).unwrap();
        let two = *self.index_map.get(link.dst.device.as_str()).unwrap();
        let key = (one, two);
        let mut result = MergeResult::ADDED(());
        if let Some((exist_link, exist_cost)) = self.link_map.get(&key) {
            if exist_link != link && *exist_cost < cost {
                return MergeResult::CONFLICT;
            } else {
                result = MergeResult::MERGED;
            }
        }
        self.link_map.insert(key, ((*link).clone(), cost));
        self.connectivity.set(one, two, cost);
        result
    }

    pub fn get_path(&self, src: &str, dst: &str) -> Option<Path> {
        let src = *self.index_map.get(src).unwrap();
        let dst = *self.index_map.get(dst).unwrap();

        let mut dist = HashMap::new();
        dist.insert(src, 0);
        let mut prev = HashMap::new();
        let mut visited = HashSet::new();

        let mut vertx_heap = BinaryHeap::new();
        vertx_heap.push(ReversePrioritywithVertx {
            vertx: src,
            priority: 0,
        });

        while let Some(ReversePrioritywithVertx { vertx, priority }) = vertx_heap.pop() {
            if !visited.insert(vertx) {
                continue;
            }

            for i in 0..self.node_count {
                if *self.connectivity.get(vertx, i) != 0 {
                    let cost = *self.connectivity.get(vertx, i);
                    let new_dist = priority + cost;
                    let is_shorter = dist.get(&i).map_or(true, |&current| new_dist < current);

                    if is_shorter {
                        dist.insert(i, new_dist);
                        prev.insert(i, vertx);
                        vertx_heap.push(ReversePrioritywithVertx {
                            vertx: i,
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
                let key = (one, two);
                let (link, cost) = self.link_map.get(&key).unwrap();
                lists.push(link.clone());
                c += cost;
                if one == src {
                    break;
                }
                two = one;
                one = *prev.get(&two).unwrap();
            }
            lists.reverse();
            Some(Path { links: lists })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Path {
    pub links: Vec<Link>,
}

#[derive(Eq)]
struct ReversePrioritywithVertx {
    pub vertx: usize,
    pub priority: u8,
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

fn test_incr() {
    let mut graph = GraphBase::with_capacity(10);
    graph.incr_space(3);
    for y in 0..3 {
        for x in 0..3 {
            graph.set(x, y, x + y * 3);
        }
    }
    graph.incr_space(2);
    for x in 0..3 {
        for y in 0..3 {
            println!("checking x={}, y={}", x, y);
            assert_eq!(x + y * 3, *graph.get(x, y));
        }
    }
}

#[test]
fn test_defaultgraph() {
    let mut graph: DefaultGraph = DefaultGraph::new();
    let d0 = Device {
        name: "0".to_string(),
        ports: Default::default(),
        typ: DeviceType::VIRTUAL,
        device_id: 0,
        index: 0,
    };
    let cp0 = ConnectPoint {
        device: "0".to_string(),
        port: 1,
    };
    let d1 = Device {
        name: "1".to_string(),
        ports: Default::default(),
        typ: DeviceType::VIRTUAL,
        device_id: 0,
        index: 0,
    };
    let cp1 = ConnectPoint {
        device: "1".to_string(),
        port: 1,
    };
    let d2 = Device {
        name: "2".to_string(),
        ports: Default::default(),
        typ: DeviceType::VIRTUAL,
        device_id: 0,
        index: 0,
    };
    let cp2 = ConnectPoint {
        device: "2".to_string(),
        port: 1,
    };
    let d3 = Device {
        name: "3".to_string(),
        ports: Default::default(),
        typ: DeviceType::VIRTUAL,
        device_id: 0,
        index: 0,
    };
    let cp3 = ConnectPoint {
        device: "3".to_string(),
        port: 1,
    };
    let d4 = Device {
        name: "4".to_string(),
        ports: Default::default(),
        typ: DeviceType::VIRTUAL,
        device_id: 0,
        index: 0,
    };
    let cp4 = ConnectPoint {
        device: "4".to_string(),
        port: 1,
    };
    graph.add_device(&d0);
    graph.add_device(&d1);
    graph.add_device(&d2);
    graph.add_device(&d3);
    graph.add_device(&d4);
    let link = Link {
        src: cp0.clone(),
        dst: cp1.clone(),
    };
    graph.add_link(
        &Link {
            src: cp0.clone(),
            dst: cp1.clone(),
        },
        10,
    );
    graph.add_link(
        &Link {
            src: cp0.clone(),
            dst: cp3.clone(),
        },
        5,
    );
    graph.add_link(
        &Link {
            src: cp1.clone(),
            dst: cp3.clone(),
        },
        3,
    );
    graph.add_link(
        &Link {
            src: cp1.clone(),
            dst: cp2.clone(),
        },
        1,
    );
    graph.add_link(
        &Link {
            src: cp2.clone(),
            dst: cp4.clone(),
        },
        4,
    );
    graph.add_link(
        &Link {
            src: cp3.clone(),
            dst: cp2.clone(),
        },
        9,
    );
    graph.add_link(
        &Link {
            src: cp3.clone(),
            dst: cp4.clone(),
        },
        2,
    );
    graph.add_link(
        &Link {
            src: cp4.clone(),
            dst: cp0.clone(),
        },
        7,
    );
    dbg!(graph.get_path(&d0, &d2));
}
