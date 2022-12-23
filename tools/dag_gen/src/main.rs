
use std::process::{Command, Stdio};
use execute::Execute;

fn main() {

    const FFMPEG_PATH: &str = "/home/ksenia/vsc/test_dag_generators/daggen/daggen";

    let mut command = Command::new(FFMPEG_PATH);

    command.arg("--dot");
    command.arg("-o");
    command.arg("new_graph.dot");

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let output = command.execute_output().unwrap();

    if let Some(exit_code) = output.status.code() {
        if exit_code == 0 {
            println!("Ok.");
        } else {
            eprintln!("Failed.");
        }
    } else {
        eprintln!("Interrupted!");
    }

    println!("{}", String::from_utf8(output.stdout).unwrap());
    println!("{}", String::from_utf8(output.stderr).unwrap());
}