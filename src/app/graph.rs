use bitfield::fmt::Display;

// index = x+y*n
pub struct GraphBase<T> {
    pub base: Vec<T>,
    n_capacity: usize,
    n_usage: usize
}

impl<T> GraphBase<T>
    where T:Default + Copy + Display
{
    pub fn with_capacity(n:usize) -> GraphBase<T> {
        GraphBase {
            base: vec![T::default();n*n],
            n_capacity: n,
            n_usage: 0
        }
    }

    pub fn with_capacity_and_usage(cap:usize,usage:usize) -> GraphBase<T> {
        assert!(usage<=cap);
        GraphBase {
            base: vec![T::default();cap*cap],
            n_capacity: cap,
            n_usage: usage
        }
    }

    pub fn get(&self, x:usize, y:usize) -> &T {
        assert!(x<self.n_usage);
        assert!(y<self.n_usage);
        self.base.get(x+y*self.n_usage).unwrap()
    }

    pub fn get_mut(&mut self, x:usize, y:usize) -> &mut T {
        assert!(x<self.n_usage);
        assert!(y<self.n_usage);
        self.base.get_mut(x+y*self.n_usage).unwrap()
    }

    pub fn set(&mut self, x:usize, y:usize ,item:T) -> Result<(),()> {
        assert!(x<self.n_usage);
        assert!(y<self.n_usage);
        assert!(x+y*self.n_usage < self.n_capacity);
        self.base.insert(x+y*self.n_usage, item);

        Ok(())
    }

    fn reserve(&mut self, n:usize) {
        let new_cap = self.n_capacity+n;
        let addition = new_cap*new_cap - self.n_capacity*self.n_capacity;
        self.base.reserve(addition);
        self.n_capacity = new_cap;
    }

    pub fn incr_space(&mut self, n:usize) {
        assert!(n+self.n_usage<=self.n_capacity);
        let new_n_usage = self.n_usage+n;
        if self.n_usage == 0 {
            self.n_usage = new_n_usage;
            return;
        }
        for y in (0..self.n_usage).rev() {
            for x in (0..self.n_usage).rev() {
                let old_index = x+y*self.n_usage;
                let new_index = old_index+y*n;
                self.base[new_index] = self.base[old_index];
            }
        }
        self.n_usage = new_n_usage;
    }

    pub fn map_from<A,F>(other:&GraphBase<A>,map:F) -> GraphBase<T>
        where F:Fn(&A)->T
    {
        let n = other.n_usage;
        let mut new_vec = Vec::with_capacity(other.n_capacity);
        for i in 0..n*n {
            new_vec.push(map(other.base.get(i).unwrap()));
        }
        GraphBase {
            base: new_vec,
            n_capacity: other.n_capacity,
            n_usage: other.n_usage
        }
    }
}

struct ShortestPathBuffer {
    base:GraphBase<u8>,
    dirty:bool
}

pub struct Graph {
    connectivity:GraphBase<bool>,
    connectivity_path_buffer:Option<ShortestPathBuffer>
}

pub struct ShortestPath {
    one: usize,
    two: usize,
    path_sig: u128
}

pub struct ShortestPathStream {

}

impl Graph {
    pub fn new() -> Graph {
        Graph {
            connectivity:GraphBase::with_capacity_and_usage(10,5),
            connectivity_path_buffer: None
        }
    }

    pub fn add_link(&mut self,one:usize, two:usize) {
        self.connectivity.set(one,two,true);
        self.connectivity.set(two,one,true);
        if let Some(buffer) = self.connectivity_path_buffer.as_mut() {
            buffer.dirty=true;
        }
    }

    pub fn get_path(&mut self, one:usize, two:usize) -> ShortestPath {
        let mut buffer:GraphBase<u8> = GraphBase::map_from(&self.connectivity,|x|{
            if *x { 1 }
            else { 0 }
        });

        let n = buffer.n_usage;
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    let a = buffer.get(i,j);
                    let b = buffer.get(k,j);
                    let c = buffer.get(i,j);
                    if a+b<*c {
                        buffer.set(i,j,a+b);
                    }
                }
            }
        }
        unimplemented!()
    }

    pub fn get_path_stream(&self, one:usize, two:usize) -> (ShortestPath,ShortestPathStream) {
        unimplemented!()
    }
}

fn test_incr() {
    let mut graph = GraphBase::with_capacity(10);
    graph.incr_space(3);
    for y in 0..3 {
        for x in 0..3 {
            graph.set(x,y, x+y*3);
        }
    }
    graph.incr_space(2);
    for x in 0..3 {
        for y in 0..3 {
            println!("checking x={}, y={}",x,y);
            assert_eq!(x+y*3, *graph.get(x,y));
        }
    }
}