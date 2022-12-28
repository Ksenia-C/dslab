use execute::Execute;
use std::process::{Command, Stdio};

use plotters::prelude::*;
use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::time::Instant;

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
            rc!(refcell!(
                PeftScheduler::new().with_data_transfer_strategy(DataTransferStrategy::Eager)
            ))
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

    let dag_path = format!("{}/{}", test_folder, file_name);

    let dag = if dag_path.as_str().ends_with(".yaml") {
        DAG::from_yaml(dag_path.as_str())
    } else {
        DAG::from_dot(dag_path.as_str())
    };

    let resource_path = format!("{}_resources/ordinary.yaml", test_folder);
    sim.load_resources(resource_path.as_str());

    let runner = sim.init(dag);
    runner.borrow_mut().enable_trace_log(true);

    let t = Instant::now();
    sim.step_until_no_events();
    println!("Makespan: {:.5}", sim.time());
    // println!(
    //     "Processed {} events in {:.2?} ({:.0} events/sec)",
    //     sim.event_count(),
    //     t.elapsed(),
    //     sim.event_count() as f64 / t.elapsed().as_secs_f64()
    // );
    // let vec_tmp: Vec<&str> = file_name.split('.').collect();
    // println!("{:?}", vec_tmp);

    // let clean_file = vec_tmp.get(0).unwrap();
    let clean_file = &file_name[0..file_name.len() - 4];
    runner.borrow().validate_completed();
    runner
        .borrow()
        .trace_log()
        .save_to_file(format!("traces1/{}_{}.log", clean_file, schedule_name).as_str())
        .unwrap();

    sim.time()
}

const NUMBER_OF_EXPERIMENT: i32 = 20;
const  LOW: f64 = 50.0;
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
    let n_start = 20;

    let mut result: Vec<String> = Vec::new();
    for i in 0..NUMBER_OF_EXPERIMENT {
        let mut command = Command::new(FFMPEG_PATH);
        command.arg("/home/ksenia/dslab/tools/dag_gen/dag_gen.py");
        command.arg(path);
        command.arg("--fat");
        command.arg("0.2");
        command.arg("--ccr");
        
        command.arg(((i as f64 / NUMBER_OF_EXPERIMENT as f64 * (HIGH - LOW) + LOW) as i64).to_string());
        command.arg("-n");
        // command.arg((n_start + (n_start as f32 * i as f32 / NUMBER_OF_EXPERIMENT as f32).floor() as i32).to_string());
        command.arg(n_start.to_string());
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
        for gen_line in String::from_utf8(output.stdout).unwrap().split('\n') {
            let filename = gen_line.split(' ').skip(1).next().unwrap_or(" ");
            if filename == " " {
                continue;
            }
            result.push(filename.to_string());
        }
    }
    return result;
}

fn main() {
    Builder::from_default_env()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .init();

    let args = Args::parse();
    let path_to_graphs = gen_graphs();

    let network_model = Rc::new(RefCell::new(ConstantBandwidthNetwork::new(1.0, 0.0)));

    let data_transfer_mode = DataTransferMode::Direct;
    // let paths = fs::read_dir(args.test_folder.as_str()).unwrap();

    let mut max_makespan_value: f64 = 0.0;
    let mut makespanes_values: Vec<f64> = Vec::new();
    for file_name in path_to_graphs {
        // let file_name = path.unwrap().file_name().clone();
        // println!("Process scehduler {}", args.scheduler_basic);
        println!("{:?}", file_name);
        let time_basic = run_scheduler(
            args.scheduler_basic.as_str(),
            args.test_folder.as_str(),
            file_name.as_str(),
            network_model.clone(),
            data_transfer_mode.clone(),
        );
        // println!("has markspace {}", time_basic);
        makespanes_values.push(time_basic);
        max_makespan_value = max_makespan_value.max(time_basic);
        println!("====================");
    }

    // drawing as graphic
    let picture_name = format!("{}.png", args.scheduler_basic);
    let root_area = BitMapBackend::new(&picture_name, (600, 700)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();

    let mut ctx = ChartBuilder::on(&root_area)
        .set_label_area_size(LabelAreaPosition::Left, 40)
        .set_label_area_size(LabelAreaPosition::Bottom, 40)
        .caption("Scatter Demo", ("sans-serif", 40))
        .build_cartesian_2d(LOW..HIGH, 0.0..max_makespan_value.ln())
        .unwrap();

    ctx.configure_mesh().draw().unwrap();

    ctx.draw_series(
        LineSeries::new(
            (0..)
                .zip(makespanes_values.iter())
                .map(|(idx, makespan)| (idx as f64 / NUMBER_OF_EXPERIMENT as f64 * (HIGH - LOW) + LOW, (*makespan).ln())), // The data iter
            &RED, // Make the series opac
        ), // Make a brighter border
    )
    .unwrap();
}
