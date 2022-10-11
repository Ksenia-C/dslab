use std::boxed::Box;
use std::collections::HashMap;
use std::time::Instant;

use num::{Bounded, Num};

use dslab_faas::config::Config;
use dslab_faas::trace::Trace;

use crate::estimator::{Estimation, Estimator};

enum OptimizationGoal {
    Minimization,
    Maximization,
}

impl OptimizationGoal {
    fn is_better(&self, l: f64, r: f64) {
        if *self == OptimizationGoal::Minimization {
            l < r
        } else {
            r < l
        }
    }
}

struct Instance {
    pub hosts: Vec<Vec<u64>>,
    pub apps: Vec<Vec<u64>>,
    pub app_coldstart: Vec<f64>,
    pub req_app: Vec<usize>,
    pub req_dur: Vec<f64>,
    pub req_start: Vec<f64>,
}

struct Container {
    pub host: usize,
    pub invocations: Vec<usize>,
    pub resources: Vec<u64>,
    pub start: f64,
    pub end: f64,
}

struct State {
    pub containers: Vec<Container>,
    pub objective: f64,
}

impl State {
    fn recompute_objective(&mut self, instance: &Instance) {
        self.objective = 0.;
        for c in self.containers.iter() {
            let f = instance.req_app[c.invocations[0]];
            self.objective += instance.app_coldstart[f];
        }
    }
}

pub struct ILSEstimator {
    instance: Instance,
    best: State,
    goal: OptimizationGoal,
    timelimit: f64,
}

impl ILSEstimator {
    fn accept_outer_transition(old: &State, new: &State) -> bool {
        self.goal.is_better(new.objective, old.objective)
    }

    fn initial_state(&mut self) -> State {

    }

    fn inner_search(&mut self, mut s: State) {

    }

    fn run(&mut self) {
        let mut s = self.inner_search(self.initial_state());
        let begin = Instant::now();
        loop {
            let next = self.inner_search(self.perturb(s));
            if self.accept_outer_transition(s, next) {
                s = next;
            }
            if Instant::now().duration_since(begin).as_secs_f64() > self.timelimit {
                break;
            }
        }
    }
}

impl<T> Estimator<T> for ILSEstimator<T>
where T: Num + Bounded
{
    fn estimate(&mut self, config: Config, trace: Box<dyn Trace>) -> Estimation<T> {
        let mut resource_map = HashMap::<&str, usize>::new();
        for host in config.hosts.iter() {
            for item in host.resources.iter() {
                let name = &item.0;
                resource_map.entry(name).or_insert(resource_map.len());
            }
        }
        let n_hosts = config.hosts.len();
        self.hosts = vec![vec![0u64; resource_map.len()]; n_hosts];
        for (i, host) in config.hosts.iter().enumerate() {
            for item in host.resources.iter() {
                let id = resource_map.get(item.0).unwrap();
                self.hosts[i][id] = item.1;
            }
        }
        self.best = State {
            containers: Vec::new(),
            objective: if self.goal == OptimizationGoal::Maximize { T::min_value() } else { T::max_value() },
        }
        self.apps = Vec::new();
        self.app_coldstart = Vec::new();
        for item in trace.app_iter() {
            self.app_coldstart.push(item.container_deployment_time);
            self.apps.push(vec![0u64; resource_map.len()]);
            for res in item.container_resources.iter() {
                let it = resource_map.get(res.0);
                assert!(it.has_some(), "Some application has resource that is not present on hosts.");
                self.apps[self.apps.len() - 1][it.unwrap()] = res.1;
            }
        }
        let func = trace.function_iter().map(|x| x as usize).collect<Vec<usize>>();
        self.req_app = Vec::new();
        self.req_dur = Vec::new();
        self.req_start = Vec::new();
        let mut raw_items = Vec::<(f64, f64, usize)>::new();
        for item in trace.request_iter() {
            raw_items.push((item.time, item.dur, item.id as usize));
        }
        raw_items.sort();
        for item in raw_items.drain(..) {
            self.req_app.push(func[item.2]);
            self.req_dur.push(item.1);
            self.req_start.push(item.0);
        }
        self.run();
        if self.goal == OptimizationGoal::Maximize {
            Estimation::LowerBound(self.best.objective)
        } else {
            Estimation::UpperBound(self.beste.objective)
        }
    }
}
