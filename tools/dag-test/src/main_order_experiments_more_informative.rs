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
use dslab_dag::schedulers::heft_oct::HeftOctScheduler;
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
        "heft_oct" => {
            rc!(refcell!(
                HeftOctScheduler::new().with_data_transfer_strategy(DataTransferStrategy::Eager),
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

    let file_name = file_path.split('/').collect::<Vec<&str>>();
    runner.borrow().validate_completed();

    *task_order = runner.borrow().task_order();
    (sim.time(), lower_bound)
}

const AVAILABLE_SCHEDULERS: [&str; 4] = ["heft", "dls", "peft", "heft_oct"]; //  "heft_new","lookahead", "dls_art"
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

#[derive(Debug)]
struct SavedCase {
    resouce: String,
    job_name: String,
    what: String,
}

#[derive(Clone)]
struct NamedOrder {
    task_order: Vec<usize>,
    job_name: String,
}

use std::sync::atomic::{AtomicUsize, Ordering};

fn main_test_dataset(args: Arc<Args>) {
    let schedulers_set = if args.is_first_run == "true" {
        AVAILABLE_SCHEDULERS.to_vec()
    } else {
        TESTING_SCHEDULERS.to_vec()
    };

    let best_arrange = Arc::new(Mutex::new(HashMap::<String, Vec<String>>::new()));

    let data_transfer_mode = DataTransferMode::Direct;

    let path_to_graphs: Vec<String> = fs::read_dir(&args.graphs_folder)
        .unwrap()
        .map(|x| x.unwrap().path().display().to_string())
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

    let mut compare_table = HashMap::<String, HashMap<String, usize>>::new();

    for sched1 in schedulers_set.iter() {
        let sched = String::from(*sched1);
        compare_table.insert(sched.clone(), HashMap::new());
        let insert_table = compare_table.get_mut(&sched).unwrap();
        for sched2 in schedulers_set.iter() {
            insert_table.insert(String::from(*sched2), 0);
        }
    }

    let compare_table = Arc::new(Mutex::new(compare_table));

    let path_to_resources = Arc::new(path_to_resources);

    let reorder_helped_cnt = Arc::new(AtomicUsize::new(0));

    let all_interesting_cases = Arc::new(Mutex::new(Vec::<SavedCase>::new()));

    for _ in 0..THREAD_CNT {
        let (chunk_work_this, new_chunks_work) = chunks_work.split_at_mut(1);

        let chunk_work_this = chunk_work_this.to_vec();
        chunks_work = new_chunks_work.to_vec();

        thread_handles.push(thread::spawn({
            let schedulers_set = schedulers_set.clone();
            let best_arrange = best_arrange.clone();
            let path_to_resources = path_to_resources.clone();
            let all_interesting_cases = all_interesting_cases.clone();
            let reorder_helped_cnt = reorder_helped_cnt.clone();
            let compare_table = compare_table.clone();
            move || {
                let mut cnt_maid = 0;
                let mut interesting_cases = Vec::<SavedCase>::new();

                for file_name in chunk_work_this.last().unwrap().iter() {
                    cnt_maid += 1;

                    if cnt_maid % 10 == 0 {
                        let stdout = io::stdout();
                        let _ = writeln!(&mut stdout.lock(), "{}", cnt_maid);
                    }

                    let mut task_orders_over_resources = Vec::new();

                    let mut resouce_order: Vec<String> = Vec::<String>::new();

                    for resource_path in path_to_resources.iter() {
                        resouce_order.push(resource_path.clone());

                        let resources = read_resource_configs(&resource_path);
                        let network = read_network_config(&resource_path);

                        let mut local_best_arrange = HashMap::<String, Vec<String>>::new();

                        let mut scdulers_orders = HashMap::new();
                        let mut scdulers_times = HashMap::new();
                        let (mut time_best, mut sched_best) = (-1.0, "");

                        let job_name_ = file_name.split('/').last().unwrap();
                        let job_name = &job_name_[0..job_name_.len() - 4];

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
                            scdulers_orders.insert(
                                scheduler.to_string(),
                                NamedOrder {
                                    task_order: task_order,
                                    job_name: job_name.to_string(),
                                },
                            );
                            scdulers_times.insert(*scheduler, time_basic);
                        }

                        let mut compare_table_ = compare_table.lock().unwrap();
                        for sched1 in schedulers_set.iter() {
                            let sched = String::from(*sched1);
                            let insert_table = compare_table_.get_mut(&sched).unwrap();
                            for sched2 in schedulers_set.iter() {
                                if scdulers_times[&*sched] < scdulers_times[sched2] {
                                    *insert_table.get_mut(*sched2).unwrap() += 1;
                                }
                            }
                        }

                        let best_sched_list = local_best_arrange
                            .entry(String::from(sched_best))
                            .or_insert_with(|| Vec::new());
                        best_sched_list.push(format!("{} at {}", job_name, resource_path));

                        let mut best_order: NamedOrder = (*scdulers_orders.get(sched_best).unwrap()).clone();

                        if sched_best == "peft" || sched_best == "dls" {
                            println!("try run heft with same orders from {}", sched_best);
                            let scheduler = "heft";

                            let save_best_task_order = best_order.task_order.clone();

                            let (time_basic, _) = run_scheduler(
                                scheduler.as_str(),
                                file_name.as_str(),
                                resources.clone(),
                                network.clone(),
                                data_transfer_mode.clone(),
                                None,
                                &mut best_order.task_order,
                                true,
                            );

                            if time_best != time_basic {
                                interesting_cases.push(SavedCase {
                                    resouce: resource_path.clone(),
                                    job_name: best_order.job_name.clone(),
                                    what: format!(
                                        "Order from {} didn't help, {}!={}, order from best {:?}, order from heft {:?}",
                                        sched_best,
                                        time_best,
                                        time_basic,
                                        save_best_task_order,
                                        scdulers_orders.get("heft" as &str).unwrap().task_order
                                    ),
                                })
                            } else {
                                reorder_helped_cnt.fetch_add(1, Ordering::SeqCst);
                            }
                        }
                        task_orders_over_resources.push(scdulers_orders);

                        let mut best_arrange_ = (*best_arrange).lock().unwrap();

                        for (key, mut value) in local_best_arrange.iter_mut() {
                            let additive = best_arrange_.entry(key.to_string()).or_insert_with(|| Vec::new());
                            additive.append(&mut value);
                        }
                    }

                    for scheduler in schedulers_set.iter() {
                        let orders: Vec<&NamedOrder> = task_orders_over_resources
                            .iter()
                            .map(|x| x.get(*scheduler).unwrap())
                            .collect();
                        for i in 1..orders.len() {
                            assert!(orders[i].job_name == orders[i - 1].job_name);
                            if orders[i].task_order != orders[i - 1].task_order {
                                interesting_cases.push(SavedCase {
                                    resouce: resouce_order[i].clone(),
                                    job_name: orders[i].job_name.clone(),
                                    what: format!(
                                        "Order different between resource conf, {} -> {}",
                                        resouce_order[i],
                                        resouce_order[i - 1]
                                    ),
                                })
                            }
                        }
                    }
                }

                let mut all_interesting_cases_ = all_interesting_cases.lock().unwrap();
                all_interesting_cases_.append(&mut interesting_cases);
            }
        }));
    }
    assert!(chunks_work.len() == 0);

    for h in thread_handles {
        h.join().unwrap();
    }

    let compare_table = compare_table.lock().unwrap();

    let best_arrange = (*best_arrange).lock().unwrap();

    let path = Path::new(&args.result_directory).join("meta_runs");
    let mut meta_file = match File::create(&path) {
        Err(why) => panic!("cant create file {}", why),
        Ok(meta_file) => meta_file,
    };

    for (sched, jobs) in best_arrange.iter() {
        let mut output = format!("{} {}\n", sched, jobs.len());

        for el in jobs.iter() {
            output += &format!("{}\n", el);
        }
        meta_file.write_all(output.as_bytes()).unwrap();
    }

    let all_interesting_cases = all_interesting_cases.lock().unwrap();
    for case in all_interesting_cases.iter() {
        let buf = format!("{} - {}\n\t{}\n", case.job_name, case.resouce, case.what);
        meta_file.write_all(buf.as_bytes()).unwrap();
    }

    println!("helped to {:?}", reorder_helped_cnt);

    println!("compare_table final ");
    for sched1 in schedulers_set.iter() {
        for sched2 in schedulers_set.iter() {
            println!(
                "{} better {} at {} cases",
                sched1, sched2, compare_table[*sched1][*sched2]
            );
        }
    }
}

fn main() {
    Builder::from_default_env()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .init();
    let args = Args::parse();

    load_dls_art_coefs("./dls_art_coefs");
    main_test_dataset(Arc::new(args))
}
