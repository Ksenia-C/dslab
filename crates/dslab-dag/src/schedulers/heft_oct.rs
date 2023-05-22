/*
heft range + peft resource selection
*/

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};

use dslab_core::context::SimulationContext;
use dslab_core::log_warn;
use dslab_core::Id;

use crate::dag::DAG;
use crate::data_item::{DataTransferMode, DataTransferStrategy};
use crate::runner::Config;
use crate::scheduler::{Action, Scheduler, SchedulerParams, TimeSpan};
use crate::schedulers::common::*;
use crate::schedulers::treap::Treap;
use crate::system::System;

pub struct HeftOctScheduler {
    data_transfer_strategy: DataTransferStrategy,
    pub task_order: Cell<Vec<usize>>,
}

impl HeftOctScheduler {
    pub fn new() -> Self {
        Self {
            data_transfer_strategy: DataTransferStrategy::Eager,
            task_order: Cell::new(Vec::new()),
        }
    }

    pub fn from_params(params: &SchedulerParams) -> Self {
        Self {
            data_transfer_strategy: params
                .get("data_transfer_strategy")
                .unwrap_or(DataTransferStrategy::Eager),
            task_order: Cell::new(Vec::new()),
        }
    }

    pub fn with_data_transfer_strategy(mut self, data_transfer_strategy: DataTransferStrategy) -> Self {
        self.data_transfer_strategy = data_transfer_strategy;
        self
    }

    pub fn with_task_order(mut self, task_order: Vec<usize>) -> Self {
        self.task_order.set(task_order);
        self
    }

    fn calc_opt(&self, dag: &DAG, system: System, config: &Config, ctx: &SimulationContext) -> Vec<Vec<f64>> {
        let resources = system.resources;
        let network = system.network;

        let task_count = dag.get_tasks().len();
        // optimistic cost table
        let mut oct = vec![vec![0.0; resources.len()]; task_count];
        for task_id in topsort(dag).into_iter().rev() {
            for resource_id in 0..resources.len() {
                oct[task_id][resource_id] = task_successors(task_id, dag)
                    .into_iter()
                    .map(|(succ, weight)| {
                        (0..resources.len())
                            .map(|succ_resource| {
                                oct[succ][succ_resource]
                                    + dag.get_task(succ).flops / resources[succ_resource].speed
                                    + weight * {
                                        config.data_transfer_mode.net_time(
                                            network,
                                            resources[resource_id].id,
                                            resources[succ_resource].id,
                                            ctx.id(),
                                        )
                                    }
                            })
                            .min_by(|a, b| a.total_cmp(b))
                            .unwrap()
                    })
                    .max_by(|a, b| a.total_cmp(b))
                    .unwrap_or(0.);
            }
        }
        oct
    }

    fn schedule(&self, dag: &DAG, system: System, config: Config, ctx: &SimulationContext) -> Vec<Action> {
        let resources = system.resources;
        let network = system.network;

        let avg_net_time = system.avg_net_time(ctx.id(), &config.data_transfer_mode);

        let task_count = dag.get_tasks().len();

        let task_ranks = calc_ranks(system.avg_flop_time(), avg_net_time, dag);
        let mut task_ids = (0..task_count).collect::<Vec<_>>();
        task_ids.sort_by(|&a, &b| task_ranks[b].total_cmp(&task_ranks[a]));
        // let task_ids = self.sort_tasks(dag, system, avg_net_time);
        // task_ids = vec!;

        let oct = self.calc_opt(dag, system, &config, ctx);

        if unsafe { (*self.task_order.as_ptr()).len() } == 0 {
            self.task_order.set(task_ids.clone());
        } else {
            task_ids = self.task_order.take();
        }

        let mut task_finish_times = vec![0.; task_count];
        let mut scheduled_tasks: Vec<Vec<BTreeSet<ScheduledTask>>> = resources
            .iter()
            .map(|resource| (0..resource.cores_available).map(|_| BTreeSet::new()).collect())
            .collect();
        let mut memory_usage: Vec<Treap> = (0..resources.len()).map(|_| Treap::new()).collect();
        let mut data_locations: HashMap<usize, Id> = HashMap::new();
        let mut task_locations: HashMap<usize, Id> = HashMap::new();

        let mut result: Vec<(f64, Action)> = Vec::new();

        for task_id in task_ids.into_iter() {
            let mut best_finish = -1.;
            let mut best_start = -1.;
            let mut best_resource = 0;
            let mut best_oeft = -1.;
            let mut best_cores: Vec<u32> = Vec::new();
            for resource in 0..resources.len() {
                let res = evaluate_assignment(
                    task_id,
                    resource,
                    &task_finish_times,
                    &scheduled_tasks,
                    &memory_usage,
                    &data_locations,
                    &task_locations,
                    &self.data_transfer_strategy,
                    dag,
                    resources,
                    network,
                    &config,
                    ctx,
                );
                if res.is_none() {
                    continue;
                }
                let (start_time, finish_time, cores) = res.unwrap();

                let oeft = finish_time + oct[task_id][resource];

                if best_oeft == -1. || best_oeft > oeft {
                    best_start = start_time;
                    best_finish = finish_time;
                    best_oeft = oeft;
                    best_resource = resource;
                    best_cores = cores;
                }
            }

            assert_ne!(best_finish, -1.);

            task_finish_times[task_id] = best_finish;
            for &core in best_cores.iter() {
                scheduled_tasks[best_resource][core as usize].insert(ScheduledTask::new(
                    best_start,
                    best_finish,
                    task_id,
                ));
            }
            memory_usage[best_resource].add(best_start, best_finish, dag.get_task(task_id).memory);
            for &output in dag.get_task(task_id).outputs.iter() {
                data_locations.insert(output, resources[best_resource].id);
            }
            task_locations.insert(task_id, resources[best_resource].id);

            result.push((
                best_start,
                Action::ScheduleTaskOnCores {
                    task: task_id,
                    resource: best_resource,
                    cores: best_cores,
                    expected_span: Some(TimeSpan::new(best_start, best_finish)),
                },
            ));
        }

        result.sort_by(|a, b| a.0.total_cmp(&b.0));
        result.into_iter().map(|(_, b)| b).collect()
    }
}

impl Scheduler for HeftOctScheduler {
    fn start(&mut self, dag: &DAG, system: System, config: Config, ctx: &SimulationContext) -> Vec<Action> {
        assert_ne!(
            config.data_transfer_mode,
            DataTransferMode::Manual,
            "HeftScheduler doesn't support DataTransferMode::Manual"
        );

        if dag.get_tasks().iter().any(|task| task.min_cores != task.max_cores) {
            log_warn!(
                ctx,
                "some tasks support different number of cores, but HEFT will always use min_cores"
            );
        }

        self.schedule(dag, system, config, ctx)
    }

    fn is_static(&self) -> bool {
        true
    }

    fn get_task_order(&self) -> Vec<usize> {
        return self.task_order.take();
    }
}

impl Default for HeftOctScheduler {
    fn default() -> Self {
        Self::new()
    }
}
