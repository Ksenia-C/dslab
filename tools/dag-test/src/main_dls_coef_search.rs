/*
cargo run -- --graphs-folder other/inss/ --mode test --result-directory alibaba_meta/other --trace-folder alibaba_meta/other/trace
 */
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use clap::Parser;
use druid::piet::TextStorage;
use druid::text::format;
use plotters::prelude::*;
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

    /// Number of experiments
    #[clap(long, default_value = "100")]
    number_of_experiment: u32,

    /// Experiment mode (hetero = compare counts and improvments, homo=dag_gen)
    #[clap(long, default_value = "homo")]
    mode: String,

    #[clap(long, default_value = "./traces1")]
    trace_folder: String,

    #[clap(long, default_value = "false")]
    is_first_run: String,
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct SchedulerStat {
    markspaces: HashMap<String, HashMap<String, (f64, String)>>,
}

impl SchedulerStat {
    fn new() -> Self {
        return SchedulerStat {
            markspaces: HashMap::new(),
        };
    }

    fn save_to_file(&self, file_name: &str) {
        let j = serde_json::to_string(&self).unwrap();

        let path = Path::new(file_name);
        let mut file = match File::create(&path) {
            Err(why) => panic!("cant open file to write {}", why),
            Ok(file) => file,
        };

        match file.write_all(j.as_bytes()) {
            Err(why) => panic!("cant save serialization {}", why),
            Ok(_) => {}
        }
    }

    fn load_from_file(&mut self, file_name: &str) {
        let path = Path::new(file_name);
        let mut file = match File::open(&path) {
            Err(why) => {
                println!(
                    "Coudn't load from file previous runs, check: {}. Why: {}",
                    file_name, why
                );
                return;
            }
            Ok(file) => file,
        };
        let mut contents = String::new();
        match file.read_to_string(&mut contents) {
            Err(why) => panic!("couldn't read: {}", why),
            Ok(_) => {}
        }

        *self = serde_json::from_str(&contents).unwrap();
    }
}

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
    trace_file: &mut String,
) -> (f64, f64) {
    let scheduler: Rc<RefCell<dyn Scheduler>> = match schedule_name {
        "simple" => rc!(refcell!(SimpleScheduler::new())),
        "simple-with-data" => rc!(refcell!(SimpleDataScheduler::new())),
        "heft" => {
            rc!(refcell!(
                HeftScheduler::new().with_data_transfer_strategy(DataTransferStrategy::Eager)
            ))
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
    runner.borrow_mut().enable_trace_log(true);

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
    *trace_file = format!("{}/{}_{}.log", *trace_file, file_name.last().unwrap(), schedule_name);
    runner.borrow().validate_completed();
    runner.borrow().trace_log().save_to_file(trace_file.as_str()).unwrap();

    (sim.time(), sim.time() / lower_bound)
}

struct SchedultedTask {
    pub name: String,
    pub makespans: Vec<f64>,
}

impl SchedultedTask {
    pub fn new(name: &str) -> SchedultedTask {
        SchedultedTask {
            name: String::from(name),
            makespans: Vec::new(),
        }
    }
    pub fn add_makespan(&mut self, new_span: f64) {
        self.makespans.push(new_span);
    }
}

struct ComperisonMakespansUnit {
    graph_name: String,
    makespan_improvments: f64,
    trace_paths: (String, String),
}

const AVAILABLE_SCHEDULERS: [&str; 4] = ["heft", "dls", "lookahead", "peft"]; // "dls_art",   "heft_new"
const TESTING_SCHEDULERS: [&str; 1] = ["dls_art"];

fn main_homogenious_features_dataset(args: Args) {
    let resource_path = format!("{}/cluster-het-4-32.yaml", args.resource_folder);

    let resources = read_resource_configs(&resource_path);
    let network = read_network_config(&resource_path);

    let data_transfer_mode = DataTransferMode::Direct;

    let path_to_graphs: Vec<String> = fs::read_dir(&args.graphs_folder)
        .unwrap()
        .map(|x| x.unwrap().path().display().to_string())
        .collect();

    let mut max_makespan_value: f64 = 0.0;
    let mut min_makespan_value: f64 = -1.0;

    let mut makespans: Vec<SchedultedTask> = Vec::new();
    makespans.push(SchedultedTask::new("heft"));
    makespans.push(SchedultedTask::new("lookahead"));
    makespans.push(SchedultedTask::new("dls"));
    makespans.push(SchedultedTask::new("peft"));

    let path = Path::new(&args.result_directory).join("meta_runs");
    let mut meta_file = match File::create(&path) {
        Err(why) => panic!("cant create file {}", why),
        Ok(meta_file) => meta_file,
    };

    let mut convergence_set: HashMap<String, HashMap<String, Vec<ComperisonMakespansUnit>>> = HashMap::new();

    for file_name in path_to_graphs {
        if !file_name.ends_with(".yaml") {
            continue;
        }
        meta_file
            .write_all((String::from(&file_name) + "\n").as_bytes())
            .unwrap();

        println!("{:?}", file_name);
        let mut local_converge: HashMap<String, (f64, String)> = HashMap::new();

        for scheduler in makespans.iter_mut() {
            let mut trace_file = args.trace_folder.clone();
            let (time_basic, _) = run_scheduler(
                scheduler.name.as_str(),
                file_name.as_str(),
                resources.clone(),
                network.clone(),
                data_transfer_mode.clone(),
                Some(&mut meta_file),
                &mut trace_file,
            );

            max_makespan_value = max_makespan_value.max(time_basic);
            if min_makespan_value == -1.0 || min_makespan_value < time_basic {
                min_makespan_value = time_basic;
            }

            scheduler.add_makespan(time_basic);
            local_converge.insert(scheduler.name.clone(), (time_basic, trace_file));
        }

        for sched1 in AVAILABLE_SCHEDULERS {
            for sched2 in AVAILABLE_SCHEDULERS {
                if local_converge[sched1] < local_converge[sched2] {
                    let values = convergence_set.entry(sched1.to_string()).or_insert(HashMap::new());

                    let value = values.entry(sched2.to_string()).or_insert(Vec::new());
                    value.push(ComperisonMakespansUnit {
                        graph_name: file_name.to_string(),
                        makespan_improvments: (local_converge[sched2].0 as f64 - local_converge[sched1].0 as f64)
                            / local_converge[sched2].0 as f64, // diff / worst_makespan
                        trace_paths: (local_converge[sched1].1.clone(), local_converge[sched2].1.clone()),
                    });
                };
            }
        }

        println!("====================");
    }

    // drawing as graphic
    let picture_name = format!("{}/schedulers.png", args.result_directory);
    let root_area = BitMapBackend::new(&picture_name, (600, 700)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption("Makespans", ("sans-serif", 40))
        .build_cartesian_2d(0..args.number_of_experiment, 0.0..max_makespan_value)
        .unwrap();

    ctx.configure_mesh().draw().unwrap();

    let colors = [&RED, &BLUE, &GREEN, &BLACK];
    for schedule in makespans.iter().enumerate() {
        let color = colors[schedule.0];
        ctx.draw_series(LineSeries::new(
            (0..)
                .zip(schedule.1.makespans.iter())
                .map(|(idx, makespan)| (idx, *makespan)),
            color,
        ))
        .unwrap()
        .label(schedule.1.name.as_str())
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color.stroke_width(3)));
    }
    ctx.configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .draw()
        .unwrap();
    println!("ok in {}", picture_name);

    meta_file
        .write_all(String::from("------------------------------\n").as_bytes())
        .unwrap();
    for sched1 in AVAILABLE_SCHEDULERS {
        for sched2 in AVAILABLE_SCHEDULERS {
            let info = convergence_set.get(sched1.as_str());
            if info.is_none() {
                continue;
            }
            let info = info.unwrap().get(sched2.as_str());
            if info.is_none() {
                continue;
            }
            let info = info.unwrap();
            println!(
                "{} > {}\tat {} cases with average improvms\t{}",
                sched1,
                sched2,
                info.len(),
                info.iter().map(|x| x.makespan_improvments).sum::<f64>() / info.len() as f64 // convergenceSet[shedie_1.as_str()][shcedi2.as_str()] / total_cnt as f64
            );
            meta_file
                .write_all(format!("{} > {}\n", sched1, sched2).as_bytes())
                .unwrap();
            for better_example in info.iter() {
                meta_file
                    .write_all(
                        format!(
                            "improvm of {}: {}\n+ {}\n- {}\n",
                            better_example.graph_name,
                            better_example.makespan_improvments,
                            better_example.trace_paths.0,
                            better_example.trace_paths.1
                        )
                        .as_bytes(),
                    )
                    .unwrap();
            }
        }
    }
}

// daggen filename format
fn main_geterogenious_features_dataset(args: Args) {
    let resource_path = format!("{}/cluster-het-4-32.yaml", args.resource_folder);
    let resources = read_resource_configs(&resource_path);
    let network = read_network_config(&resource_path);

    let data_transfer_mode = DataTransferMode::Direct;
    let path_to_graphs: Vec<String> = fs::read_dir(&args.graphs_folder)
        .unwrap()
        .map(|x| x.unwrap().path().display().to_string())
        .collect();

    let mut makespans: Vec<SchedultedTask> = Vec::new();
    makespans.push(SchedultedTask::new("heft"));
    makespans.push(SchedultedTask::new("lookahead"));
    makespans.push(SchedultedTask::new("dls"));
    makespans.push(SchedultedTask::new("peft"));

    let path = Path::new(&args.result_directory).join("experiments_result.csv");
    let mut meta_file = match File::create(&path) {
        Err(why) => panic!("cant create file {}", why),
        Ok(meta_file) => meta_file,
    };

    meta_file
        .write_all(String::from("scheduler,count,fat,regular,density,jump,ccr,makespane\n").as_bytes())
        .unwrap();

    for file_name in path_to_graphs {
        if !file_name.ends_with(".dot") {
            continue;
        }

        println!("{:?}", file_name);
        for scheduler in makespans.iter_mut() {
            let mut trace_file = args.trace_folder.clone();
            let (time_basic, _) = run_scheduler(
                scheduler.name.as_str(),
                file_name.as_str(),
                resources.clone(),
                network.clone(),
                data_transfer_mode.clone(),
                None,
                &mut trace_file,
            );
            scheduler.add_makespan(time_basic);

            let clean_file = file_name.split('/').collect::<Vec<&str>>();
            let clean_file = clean_file.last().unwrap();
            let dag_params = clean_file.split('_').collect::<Vec<&str>>();

            meta_file
                .write_all(
                    format!(
                        "{},{},{},{},{},{},{},{}\n",
                        scheduler.name,
                        dag_params[1], // count = n
                        dag_params[2], // fat
                        dag_params[3], // regular
                        dag_params[4], // density
                        dag_params[5], // jump
                        dag_params[8], // ccr
                        time_basic
                    )
                    .as_bytes(),
                )
                .unwrap();
        }

        println!("====================");
    }
}
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
    let mut sched_stat = SchedulerStat::new();
    sched_stat.load_from_file("sched_stat.json");
    let schedulers_set = if args.is_first_run == "true" {
        AVAILABLE_SCHEDULERS.to_vec()
    } else {
        TESTING_SCHEDULERS.to_vec()
    };

    for scheduler in schedulers_set.iter() {
        sched_stat.markspaces.remove(&scheduler.to_string());
    }
    let sched_stat = Arc::new(Mutex::new(sched_stat));

    let best_arrange = Arc::new(Mutex::new(HashMap::<String, Vec<String>>::new()));

    let resource_path = format!("{}/cluster-het-4-32.yaml", args.resource_folder);

    let resources = Arc::new(read_resource_configs(&resource_path));
    let network = Arc::new(read_network_config(&resource_path));

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

    let path_for_graph = Path::new(&args.result_directory).join("graphic.csv");
    let graph_file = Arc::new(Mutex::new(match File::create(&path_for_graph) {
        Err(why) => panic!("cant create file {}", why),
        Ok(meta_file) => meta_file,
    }));
    for _ in 0..THREAD_CNT {
        let (chunk_work_this, new_chunks_work) = chunks_work.split_at_mut(1);

        let chunk_work_this = chunk_work_this.to_vec();
        chunks_work = new_chunks_work.to_vec();

        let resources_ = resources.clone();
        let network_ = network.clone();
        let args_ = args.clone();

        thread_handles.push(thread::spawn({
            let sched_stat = sched_stat.clone();
            let schedulers_set = schedulers_set.clone();
            let graph_file = graph_file.clone();
            let best_arrange = best_arrange.clone();
            move || {
                let mut cnt_maid = 0;

                let mut local_best_arrange = HashMap::<String, Vec<String>>::new();

                for file_name in chunk_work_this.last().unwrap().iter() {
                    if !file_name.ends_with(".yaml") {
                        continue;
                    }

                    if cnt_maid % 10 == 0 {
                        let stdout = io::stdout();
                        let _ = writeln!(&mut stdout.lock(), "{}", cnt_maid);
                    }
                    cnt_maid += 1;

                    let job_name = file_name.split('/').last().unwrap().split('.').take(1).last().unwrap();
                    // let job_name_ = file_name.split('/').last().unwrap();
                    // let job_name = &job_name_[0..job_name_.len() - 4];

                    let mut csv_line = String::new();

                    let (mut time_best, mut sched_best) = (-1.0, "");
                    for scheduler in schedulers_set.iter() {
                        let mut trace_file = args_.trace_folder.clone();
                        let (time_basic, degradation) = run_scheduler(
                            scheduler.as_str(),
                            file_name.as_str(),
                            (*resources_).clone(),
                            (*network_).clone(),
                            data_transfer_mode.clone(),
                            None,
                            &mut trace_file,
                        );
                        csv_line += &format!("{},", degradation);
                        let mut sched_stat_ = (*sched_stat).lock().unwrap();

                        let values = sched_stat_
                            .markspaces
                            .entry(scheduler.to_string())
                            .or_insert(HashMap::new());
                        values
                            .entry(format!(
                                "{}_{}",
                                job_name,
                                args_.graphs_folder.split('/').last().unwrap()
                            ))
                            .or_insert((time_basic, trace_file));

                        if time_best < 0.0 || time_basic < time_best {
                            time_best = time_basic;
                            sched_best = scheduler;
                        }
                    }
                    let best_sched_list = local_best_arrange
                        .entry(String::from(sched_best))
                        .or_insert_with(|| Vec::new());
                    best_sched_list.push(String::from(job_name));

                    csv_line.replace_range((csv_line.len() - 1)..csv_line.len(), "\n");
                    let mut graph_file_ = (*graph_file).lock().unwrap();
                    graph_file_.write_all(csv_line.as_bytes()).unwrap();
                }
                let mut best_arrange_ = (*best_arrange).lock().unwrap();

                for (key, mut value) in local_best_arrange.iter_mut() {
                    let additive = best_arrange_.entry(key.to_string()).or_insert_with(|| Vec::new());
                    additive.append(&mut value);
                }
            }
        }));
    }
    assert!(chunks_work.len() == 0);

    for h in thread_handles {
        h.join().unwrap();
    }

    let best_arrange = (*best_arrange).lock().unwrap();

    let sched_stat = sched_stat.lock().unwrap();

    let path = Path::new(&args.result_directory).join("meta_runs");
    let mut meta_file = match File::create(&path) {
        Err(why) => panic!("cant create file {}", why),
        Ok(meta_file) => meta_file,
    };

    for (sched, jobs) in best_arrange.iter() {
        let mut output = format!("{}\n", sched);

        for el in jobs.iter() {
            output += &format!("{}\n", el);
        }
        meta_file.write_all(output.as_bytes()).unwrap();
    }

    for sched1 in AVAILABLE_SCHEDULERS {
        for sched2 in AVAILABLE_SCHEDULERS {
            let mut diff = Vec::<ComperisonMakespansUnit>::new();
            for (job_name, makespane1, makespane2) in sched_stat.markspaces[sched1].iter().filter_map(|(key, value)| {
                match sched_stat.markspaces[sched2].contains_key(key) {
                    true => Some((key, value, &sched_stat.markspaces[sched2][key])),
                    false => None,
                }
            }) {
                if makespane1.0 < makespane2.0 {
                    diff.push(ComperisonMakespansUnit {
                        graph_name: job_name.to_string(),
                        makespan_improvments: (makespane2.0 - makespane1.0) / makespane2.0, // diff / worst_makespan
                        trace_paths: (makespane1.1.clone(), makespane2.1.clone()),
                    });
                }
            }
            let mut mean_improv = 0.0;
            if diff.len() != 0 {
                mean_improv = diff.iter().map(|x| x.makespan_improvments).sum::<f64>() / diff.len() as f64;
            }

            println!(
                "{} > {}\tat {} cases with average improvms\t{}",
                sched1,
                sched2,
                diff.len(),
                mean_improv // convergenceSet[shedie_1.as_str()][shcedi2.as_str()] / total_cnt as f64
            );

            if sched1 == "dls_art" && sched2 == "dls" {
                let path = Path::new("./hyper_oprt_res");
                let mut file = match File::create(&path) {
                    Err(why) => panic!("cant open file to write {}", why),
                    Ok(file) => file,
                };
                let result_for_hyperopt = format!("{}", diff.len() as f64 * mean_improv);
                match file.write_all(result_for_hyperopt.as_bytes()) {
                    Err(why) => panic!("cant save serialization {}", why),
                    Ok(_) => {}
                }
            }

            meta_file
                .write_all(format!("{} > {}\n", sched1, sched2).as_bytes())
                .unwrap();
            for better_example in diff.iter() {
                meta_file
                    .write_all(
                        format!(
                            "improvm of {}: {}\n+ {}\n- {}\n",
                            better_example.graph_name,
                            better_example.makespan_improvments,
                            better_example.trace_paths.0,
                            better_example.trace_paths.1
                        )
                        .as_bytes(),
                    )
                    .unwrap();
            }
        }
    }

    sched_stat.save_to_file("sched_stat.json");
}

fn main() {
    Builder::from_default_env()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .init();
    let args = Args::parse();

    if Path::new(&args.trace_folder).exists() {
        fs::remove_dir_all(&args.trace_folder).unwrap();
    }
    fs::create_dir(&args.trace_folder).unwrap();

    if args.mode == "homo" {
        main_homogenious_features_dataset(args);
    } else if args.mode == "test" {
        load_dls_art_coefs("./dls_art_coefs");
        main_test_dataset(Arc::new(args))
    } else {
        main_geterogenious_features_dataset(args);
    }
}
