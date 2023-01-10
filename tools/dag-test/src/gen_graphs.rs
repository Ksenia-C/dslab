fn gen_graphs_fat() -> Vec<String> {
    const FFMPEG_PATH: &str = "/usr/bin/python";

    let path = "traces1/";
    fs::remove_dir_all(path).unwrap();
    fs::create_dir(path).unwrap();

    let path = "default/";
    fs::remove_dir_all(path).unwrap();
    fs::create_dir(path).unwrap();
    let n_start = 60;

    let mut result: Vec<String> = Vec::new();
    for i in 0..NUMBER_OF_EXPERIMENT {
        let mut command = Command::new(FFMPEG_PATH);
        command.arg("/home/ksenia/dslab/tools/dag_gen/dag_gen.py");
        command.arg(path);
        // command.arg("--regular");
        // command.arg("0.5");
        command.arg("--fat");
        command.arg((i as f32 / NUMBER_OF_EXPERIMENT as f32).to_string());
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

fn gen_graphs_regular() -> Vec<String> {
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
        command.arg("--regular");
        command.arg((i as f32 / NUMBER_OF_EXPERIMENT as f32).to_string());
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

fn gen_graphs_density() -> Vec<String> {
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
        command.arg("0.8");
        command.arg("--density");
        command.arg((i as f32 / NUMBER_OF_EXPERIMENT as f32).to_string());
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
fn gen_graphs_jump() -> Vec<String> {
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
        command.arg("0.9");
        command.arg("--jump");
        command.arg(((i as f32 / NUMBER_OF_EXPERIMENT as f32 * n_start as f32) as i64).to_string());
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
fn gen_graphs_ccr() -> Vec<String> {
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
