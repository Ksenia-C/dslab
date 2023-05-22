/*
   cargo run -- --graphs-folder tree_decr/inss_rev/ --mode test --result-directory alibaba_meta/tree_decr  --is-first-run true
*/
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use clap::Parser;
use druid::piet::TextStorage;
use std::cell::RefCell;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::rc::Rc;
use std::thread;

use sugars::{rc, refcell};

use env_logger::Builder;

use dslab_dag::dag::DAG;
use dslab_dag::dag_simulation::DagSimulation;
use dslab_dag::data_item::{DataTransferMode, DataTransferStrategy};
use dslab_dag::runner::Config;
use dslab_dag::scheduler::Scheduler;
use dslab_dag::schedulers::common::*;
use dslab_dag::schedulers::dls::DlsScheduler;
use dslab_dag::schedulers::dls_art::{CoefDescr, DlsArtScheduler};
use dslab_dag::schedulers::heft::HeftScheduler;
use dslab_dag::schedulers::heft_new::HeftNewScheduler;
use dslab_dag::schedulers::lookahead::LookaheadScheduler;
use dslab_dag::schedulers::peft::PeftScheduler;
use dslab_dag::schedulers::simple_scheduler::SimpleScheduler;
use dslab_dag::schedulers::simple_with_data::SimpleDataScheduler;

use dslab_dag::network::{read_network_config, NetworkConfig};
use dslab_dag::resource::{read_resource_configs, ResourceConfig};

#[derive(Parser, Debug)]
#[clap(about, long_about = None)]
struct Args {
    /// Take resouces from this directory
    #[clap(long, default_value = "./default_resources")]
    resource_folder: String,

    /// Take graphs from this directory
    #[clap(long, default_value = "./default")]
    graphs_folder: String,

    /// Dump results into this directory
    #[clap(long, default_value = "./default")]
    result_directory: String,

    /// Experiment mode (hetero = compare counts and improvments, homo=dag_gen)
    #[clap(long, default_value = "homo")]
    mode: String,

    #[clap(long, default_value = "false")]
    is_first_run: String,
}

use serde::{Deserialize, Serialize};

static mut DLS_ART_COEF: CoefDescr_ = CoefDescr_ {
    static_level: 1.0,
    start_time: -1.0,
    delta: 1.0,
    ds: 1.0,
};

fn run_scheduler(
    schedule_name: &str,
    file_path: &str,
    resource: Vec<ResourceConfig>,
    network: NetworkConfig,
    data_transfer_mode: DataTransferMode,
    meta_file: Option<&mut File>,
    task_order: &mut Vec<usize>,
    is_mode_set: bool,
) -> (f64, f64) {
    let scheduler: Rc<RefCell<dyn Scheduler>> = match schedule_name {
        "simple" => rc!(refcell!(SimpleScheduler::new())),
        "simple-with-data" => rc!(refcell!(SimpleDataScheduler::new())),
        "heft" => {
            rc!(refcell!(match is_mode_set {
                true => HeftScheduler::new()
                    .with_data_transfer_strategy(DataTransferStrategy::Eager)
                    .with_task_order(task_order.clone()),
                false => HeftScheduler::new().with_data_transfer_strategy(DataTransferStrategy::Eager),
            }))
        }
        "heft_new" => {
            rc!(refcell!(
                HeftNewScheduler::new().with_data_transfer_strategy(DataTransferStrategy::Eager)
            ))
        }
        "peft" => {
            rc!(refcell!(PeftScheduler::new()
                .with_data_transfer_strategy(DataTransferStrategy::Eager)
                .with_original_network_estimation()))
        }
        "lookahead" => {
            rc!(refcell!(
                LookaheadScheduler::new().with_data_transfer_strategy(DataTransferStrategy::Eager)
            ))
        }
        "dls" => {
            rc!(refcell!(
                DlsScheduler::new().with_data_transfer_strategy(DataTransferStrategy::Eager)
            ))
        }
        "dls_art" => unsafe {
            rc!(refcell!(DlsArtScheduler::new()
                .with_data_transfer_strategy(DataTransferStrategy::Eager)
                .with_other_coefs(CoefDescr {
                    static_level: DLS_ART_COEF.static_level,
                    start_time: DLS_ART_COEF.start_time,
                    delta: DLS_ART_COEF.delta,
                    ds: DLS_ART_COEF.ds
                })))
        },
        _ => {
            eprintln!("Wrong scheduler");
            std::process::exit(1);
        }
    };

    let dag_path = file_path.to_string();

    let dag = if dag_path.as_str().ends_with(".yaml") {
        DAG::from_yaml(dag_path.as_str())
    } else if dag_path.as_str().ends_with(".dot") {
        DAG::from_dot(dag_path.as_str())
    } else {
        DAG::from_wfcommons(dag_path.as_str(), 1.0)
    };

    let cp_pat_on_best = get_critical_path(&dag)
        / resource
            .iter()
            .map(|res| res.speed)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

    let best_pack_ms =
        dag.get_tasks().iter().map(|task| task.flops).sum::<f64>() / resource.iter().map(|res| res.speed).sum::<f64>();
    let lower_bound = cp_pat_on_best.max(best_pack_ms);

    let mut sim = DagSimulation::new(123, resource, network, scheduler, Config { data_transfer_mode });

    let runner = sim.init(dag);

    sim.step_until_no_events();
    match meta_file {
        Some(meta_file) => {
            match meta_file.write_all(format!("Makespan {}: {:.5}\n", schedule_name, sim.time()).as_bytes()) {
                Err(why) => panic!("colud write: {}", why),
                Ok(_) => {}
            };
        }
        None => {}
    };

    runner.borrow().validate_completed();

    *task_order = runner.borrow().task_order();
    (sim.time(), lower_bound)
}

const AVAILABLE_SCHEDULERS: [&str; 3] = ["heft", "dls", "peft"]; //  "heft_new","lookahead", "dls_art"
const TESTING_SCHEDULERS: [&str; 1] = ["dls_art"];

const THREAD_CNT: usize = 8;

#[derive(Serialize, Deserialize, Debug)]
struct CoefDescr_ {
    static_level: f64,
    start_time: f64,
    delta: f64,
    ds: f64,
}

fn load_dls_art_coefs(file_name: &str) {
    let path = Path::new(file_name);
    let mut file = match File::open(&path) {
        Err(_why) => {
            println!("Coudn't load from file previous runs, check: {}", file_name);
            return;
        }
        Ok(file) => file,
    };
    let mut contents = String::new();
    match file.read_to_string(&mut contents) {
        Err(why) => panic!("couldn't read: {}", why),
        Ok(_) => {}
    }

    unsafe { DLS_ART_COEF = serde_json::from_str(&contents).unwrap() };
}

fn main_test_dataset(args: Arc<Args>) {
    let schedulers_set = if args.is_first_run == "true" {
        AVAILABLE_SCHEDULERS.to_vec()
    } else {
        TESTING_SCHEDULERS.to_vec()
    };

    let best_arrange = Arc::new(Mutex::new(HashMap::<String, (usize, usize)>::new()));

    let data_transfer_mode = DataTransferMode::Direct;

    let path_to_graphs: Vec<String> = fs::read_dir(&args.graphs_folder)
        .unwrap()
        .map(|x| x.unwrap().path().display().to_string())
        .filter(|x| x.ends_with(".yaml") && !x.ends_with(".rev.yaml"))
        .collect();

    let mut thread_handles = Vec::new();
    let mut chunks_work: Vec<Vec<String>> = path_to_graphs
        .chunks((path_to_graphs.len() + THREAD_CNT) / THREAD_CNT)
        .map(|s| s.into())
        .collect();

    let path_to_resources: Vec<String> = fs::read_dir(&args.resource_folder)
        .unwrap()
        .map(|x| x.unwrap().path().display().to_string())
        .collect();

    let path_to_resources = Arc::new(path_to_resources);

    for _ in 0..THREAD_CNT {
        let (chunk_work_this, new_chunks_work) = chunks_work.split_at_mut(1);

        let chunk_work_this = chunk_work_this.to_vec();
        chunks_work = new_chunks_work.to_vec();

        thread_handles.push(thread::spawn({
            let args_ = args.clone();
            let schedulers_set = schedulers_set.clone();
            let best_arrange = best_arrange.clone();
            let path_to_resources = path_to_resources.clone();
            move || {
                for file_name in chunk_work_this.last().unwrap().iter() {
                    if !file_name.ends_with(".yaml") {
                        continue;
                    }

                    let mut resouce_order: Vec<String> = Vec::<String>::new();

                    for resource_path in path_to_resources.iter() {
                        resouce_order.push(resource_path.clone());

                        let resources = read_resource_configs(&resource_path);
                        let network = read_network_config(&resource_path);

                        let mut local_best_arrange = HashMap::<String, (usize, usize)>::new();

                        let mut scdulers_times = HashMap::new();
                        let (mut time_best, mut sched_best) = (-1.0, "");

                        let job_name = file_name.split('/').last().unwrap().split('.').take(1).last().unwrap();
                        // let job_name_ = file_name.split('/').last().unwrap();
                        // let job_name = &job_name_[0..job_name_.len() - 4];

                        for scheduler in schedulers_set.iter() {
                            let mut task_order = Vec::<usize>::new();

                            let (time_basic, low_bound) = run_scheduler(
                                scheduler.as_str(),
                                file_name.as_str(),
                                resources.clone(),
                                network.clone(),
                                data_transfer_mode.clone(),
                                None,
                                &mut task_order,
                                false,
                            );

                            if time_best < 0.0 || time_basic < time_best {
                                time_best = time_basic;
                                sched_best = scheduler;
                            }

                            scdulers_times.insert(scheduler, time_basic);
                        }

                        let mut task_order = Vec::<usize>::new();

                        let (time_reverse, _) = run_scheduler(
                            sched_best.as_str(),
                            format!("{}.rev.yaml", file_name.strip_suffix(".yaml").unwrap()).as_str(),
                            resources.clone(),
                            network.clone(),
                            data_transfer_mode.clone(),
                            None,
                            &mut task_order,
                            false,
                        );
                        if time_best < time_reverse {
                            local_best_arrange
                                .entry(String::from(sched_best))
                                .or_insert_with(|| (0, 0))
                                .0 += 1;
                            println!(
                                "for {} {} {} by {:.5}",
                                sched_best,
                                job_name,
                                resource_path,
                                time_reverse / time_best
                            );
                        } else if time_best > time_reverse {
                            local_best_arrange
                                .entry(String::from(sched_best))
                                .or_insert_with(|| (0, 0))
                                .1 += 1;
                            println!(
                                "rev {} {} {} by {:.5}",
                                sched_best,
                                job_name,
                                resource_path,
                                time_best / time_reverse
                            );
                        }

                        let mut best_arrange_ = (*best_arrange).lock().unwrap();
                        for (key, value) in local_best_arrange.iter_mut() {
                            let additive = best_arrange_.entry(key.to_string()).or_insert_with(|| (0, 0));
                            additive.0 += value.0;
                            additive.1 += value.1;
                        }
                    }
                }
            }
        }));
    }
    assert!(chunks_work.len() == 0);

    for h in thread_handles {
        h.join().unwrap();
    }

    let best_arrange = (*best_arrange).lock().unwrap();

    let path = Path::new(&args.result_directory).join("meta_reverse");
    let mut meta_file = match File::create(&path) {
        Err(why) => panic!("cant create file {}", why),
        Ok(meta_file) => meta_file,
    };

    for (sched, jobs) in best_arrange.iter() {
        let output = format!("{}: direct better {}, reverse better {}\n", sched, jobs.0, jobs.1);
        meta_file.write_all(output.as_bytes()).unwrap();
    }
}

fn main() {
    Builder::from_default_env()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .init();
    let args = Args::parse();
    println!("{}", args.graphs_folder);

    load_dls_art_coefs("./dls_art_coefs");
    main_test_dataset(Arc::new(args))
}
