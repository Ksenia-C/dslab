use execute::Execute;
use plotters::prelude::*;
use std::cell::RefCell;
use std::cmp;
use std::io::Write;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Instant;
use std::{thread, time};

use clap::Parser;
use std::fs;

use sugars::{rc, refcell};

use dslab_network::constant_bandwidth_model::ConstantBandwidthNetwork;
use env_logger::Builder;

use dslab_dag::dag::DAG;
use dslab_dag::dag_simulation::DagSimulation;
use dslab_dag::data_item::{DataTransferMode, DataTransferStrategy};
use dslab_dag::runner::Config;
use dslab_dag::scheduler::Scheduler;
use dslab_dag::schedulers::dls::DlsScheduler;
use dslab_dag::schedulers::heft::HeftScheduler;
use dslab_dag::schedulers::lookahead::LookaheadScheduler;
use dslab_dag::schedulers::peft::PeftScheduler;
use dslab_dag::schedulers::simple_scheduler::SimpleScheduler;
use dslab_dag::schedulers::simple_with_data::SimpleDataScheduler;

#[derive(Parser, Debug)]
#[clap(about, long_about = None)]
struct Args {
    /// Scheduler [heft, simple, simple-with-data, lookahead, dls]
    #[clap(long, default_value = "heft")]
    scheduler_basic: String,

    /// Run only one experiment
    #[clap(long, default_value = "./default")]
    test_folder: String,
}

fn run_scheduler(
    schedule_name: &str,
    test_folder: &str,
    file_name: &str,
    network_model: Rc<RefCell<ConstantBandwidthNetwork>>,
    data_transfer_mode: DataTransferMode,
) -> f64 {
    let scheduler: Rc<RefCell<dyn Scheduler>> = match schedule_name {
        "simple" => rc!(refcell!(SimpleScheduler::new())),
        "simple-with-data" => rc!(refcell!(SimpleDataScheduler::new())),
        "heft" => {
            rc!(refcell!(
                HeftScheduler::new().with_data_transfer_strategy(DataTransferStrategy::Eager)
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
        _ => {
            eprintln!("Wrong scheduler");
            std::process::exit(1);
        }
    };
    let mut sim = DagSimulation::new(123, network_model, scheduler, Config { data_transfer_mode });

    let dag_path = format!("{}.dot", file_name);

    let dag = if dag_path.as_str().ends_with(".yaml") {
        DAG::from_yaml(dag_path.as_str())
    } else if dag_path.as_str().ends_with(".dot") {
        DAG::from_dot(dag_path.as_str())
    } else {
        DAG::from_wfcommons(dag_path.as_str(), 1.0)
    };

    let resource_path = format!("{}_resources/ordinary.yaml", test_folder);
    sim.load_resources(resource_path.as_str());

    let runner = sim.init(dag);
    runner.borrow_mut().enable_trace_log(true);

    sim.step_until_no_events();
    println!("Makespan {}: {:.5}", schedule_name, sim.time());
    // println!(
    //     "Processed {} events in {:.2?} ({:.0} events/sec)",
    //     sim.event_count(),
    //     t.elapsed(),
    //     sim.event_count() as f64 / t.elapsed().as_secs_f64()
    // );
    // let vec_tmp: Vec<&str> = file_name.split('.').collect();
    // println!("{:?}", vec_tmp);

    // let clean_file = vec_tmp.get(0).unwrap();
    let clean_file = &file_name[14..file_name.len()];
    runner.borrow().validate_completed();
    runner
        .borrow()
        .trace_log()
        .save_to_file(format!("traces1/{}_{}.log", clean_file, schedule_name).as_str())
        .unwrap();

    sim.time()
}

const NUMBER_OF_EXPERIMENT: i32 = 10;
const LOW: f64 = 50.0;
const HIGH: f64 = 20000.0;
// const  LOW: f64 = 50.0;
// const HIGH: f64 = 1000.0;

fn gen_graphs() -> Vec<String> {
    const FFMPEG_PATH: &str = "/usr/bin/python";

    let path = "traces1/";
    fs::remove_dir_all(path).unwrap();
    fs::create_dir(path).unwrap();

    let path = "default/";
    fs::remove_dir_all(path).unwrap();
    fs::create_dir(path).unwrap();
    let n_start = 30;

    let mut result: Vec<String> = Vec::new();
    for i in 0..NUMBER_OF_EXPERIMENT {
        let mut command = Command::new(FFMPEG_PATH);
        command.arg("/home/ksenia/dslab/tools/dag_gen/gen_fix_size_paralel.py");
        command.arg("--file_name");
        command.arg(format!("{}workflow_{}.dot", path, i));
        command.arg("--count");
        command.arg((n_start + 2 * i).to_string());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let output = command.execute_output().unwrap();

        if let Some(exit_code) = output.status.code() {
            if exit_code == 0 {
                println!("Ok.");
            } else {
                eprintln!("Failed.");
                println!("{}", String::from_utf8(output.stdout).unwrap());
                println!("{}", String::from_utf8(output.stderr).unwrap());
                continue;
            }
        } else {
            eprintln!("Interrupted!");
        }
        println!("{}", String::from_utf8(output.stdout).unwrap());
        // println!("{}", String::from_utf8(output.stdout).unwrap());
        // for gen_line in String::from_utf8(output.stdout).unwrap().split('\n') {
        //     let filename = gen_line.split(' ').skip(1).next().unwrap_or(" ");
        //     if filename == " " {
        //         continue;
        //     }
        result.push(format!("{}workflow_{}", path, i).to_string());

        // }
    }
    return result;
}

fn load_graphs() -> Vec<String> {
    let path = "traces1/";
    fs::remove_dir_all(path).unwrap();
    fs::create_dir(path).unwrap();

    let path = "default/";

    let n_start = 24;

    let mut result: Vec<String> = Vec::new();
    for i in 0..10 {
        result.push(format!("{}workflow_{}", path, i).to_string());
    }
    return result;
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

fn main() {
    Builder::from_default_env()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .init();

    let args = Args::parse();
    let path_to_graphs = gen_graphs();
    // return;

    let network_model = Rc::new(RefCell::new(ConstantBandwidthNetwork::new(1.0, 0.0)));

    let data_transfer_mode = DataTransferMode::Direct;
    // let paths = fs::read_dir(args.test_folder.as_str()).unwrap();

    let mut max_makespan_value: f64 = 0.0;
    let mut min_makespan_value: f64 = -1.0;

    let mut makespans: Vec<SchedultedTask> = Vec::new();
    makespans.push(SchedultedTask::new("heft"));
    makespans.push(SchedultedTask::new("lookahead"));
    makespans.push(SchedultedTask::new("dls"));
    makespans.push(SchedultedTask::new("peft"));

    for file_name in path_to_graphs {
        println!("{:?}", file_name);
        for scheduler in makespans.iter_mut() {
            let time_basic = run_scheduler(
                scheduler.name.as_str(),
                args.test_folder.as_str(),
                file_name.as_str(),
                network_model.clone(),
                data_transfer_mode.clone(),
            );
            scheduler.add_makespan(time_basic);
            max_makespan_value = max_makespan_value.max(time_basic);
            if min_makespan_value == -1.0 || min_makespan_value < time_basic {
                min_makespan_value = time_basic;
            }
        }

        println!("====================");
    }
    min_makespan_value = (1.0 as f64).max(min_makespan_value - 6.0);
    max_makespan_value += 6.0;

    // drawing as graphic
    let picture_name = format!("{}.png", args.scheduler_basic);
    let root_area = BitMapBackend::new(&picture_name, (600, 700)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption("Makespans", ("sans-serif", 40))
        .build_cartesian_2d(0..NUMBER_OF_EXPERIMENT, 0.0..max_makespan_value)
        .unwrap();

    ctx.configure_mesh().draw().unwrap();

    let colors = [&RED, &BLUE, &GREEN, &BLACK];
    for schedule in makespans.iter().enumerate() {
        let color = colors[schedule.0];
        ctx.draw_series(
            LineSeries::new(
                (0..)
                    .zip(schedule.1.makespans.iter())
                    .map(|(idx, makespan)| (idx, *makespan)), // The data iter
                color, // Make the series opac
            ), // Make a brighter border
        )
        .unwrap()
        .label(schedule.1.name.as_str())
        .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color.stroke_width(3)));
    }
    ctx.configure_series_labels()
        .border_style(&BLACK)
        .background_style(&WHITE.mix(0.8))
        .draw()
        .unwrap();
}
