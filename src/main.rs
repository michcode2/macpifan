use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, RwLock},
    thread::{self, sleep},
    time::{Duration, Instant},
};

use toml::Table;

fn main() {
    let config = read_config();

    take_fan_control(&config.smc_path);

    let mut time = Instant::now();

    let integral_term = Arc::new(RwLock::new(config.initial_integral));

    let output_integral = integral_term.clone();
    let output_path = config.smc_path.clone();
    let wait_time = (config.update_interval * 1000.0) as u64;
    let log_file: PathBuf = config.output_path;
    thread::spawn(move || {
        sleep(Duration::from_secs(1));

        match fs::exists(log_file.parent().unwrap()) {
            Ok(true) => (),
            Ok(false) => fs::create_dir(log_file.parent().unwrap()).unwrap(),
            Err(_) => (),
        }

        loop {
            let mut write_string = String::new();
            if config.output_integral {
                write_string = format!("{} |", *output_integral.try_read().unwrap());
            }
            if config.output_speed {
                write_string = format!("{} {}rpm |", write_string, read_speed(&output_path));
            }
            if config.output_temperature {
                write_string = format!(
                    "{} {} deg ",
                    write_string,
                    read_max_temperature(&output_path)
                );
            }
            write_string = format!("{}\n", write_string);

            {
                let mut file = File::options()
                    .write(true)
                    .create(true)
                    .open(log_file.clone())
                    .expect("couldnt open logging file, are you root?");
                file.write(write_string.as_bytes()).unwrap();
            }

            sleep(Duration::from_millis(wait_time));
        }
    });

    loop {
        let error = read_max_temperature(&config.smc_path) - config.target_temperature;
        let speed = read_speed(&config.smc_path);
        if speed < 6000 {
            // avoid integral windup

            let temp_integral = *integral_term.try_read().unwrap();

            let mut w = integral_term.write().unwrap();
            let mut val_to_write = error as f64 * time.elapsed().as_millis() as f64;

            if temp_integral < 0.0 && val_to_write < 0.0 {
                val_to_write = 0.0
            }

            *w += val_to_write;
        }
        time = Instant::now();
        let setpoint = *integral_term.try_read().unwrap() * config.constant_integral
            + config.constant_proportional * error as f64;
        write_speed(&config.smc_path, setpoint as usize);
        sleep(Duration::from_millis(50));
    }
}

struct Config {
    smc_path: PathBuf,
    initial_integral: f64,
    constant_integral: f64,
    constant_proportional: f64,
    target_temperature: i64,
    update_interval: f64,
    output_path: PathBuf,
    output_integral: bool,
    output_temperature: bool,
    output_speed: bool,
}

fn read_config() -> Config {
    let file =
        fs::read_to_string("/etc/macpifan/macpifan.toml").expect("cant find the config file");
    let entries = file.as_str().parse::<Table>().unwrap();
    let config = Config {
        smc_path: PathBuf::from(
            entries["inout"]["smc_path"]
                .clone()
                .try_into()
                .unwrap_or("/sys/devices/platform/applesmc.768"),
        ),
        initial_integral: entries["controller_values"]["initial_integral"]
            .clone()
            .try_into()
            .unwrap_or(20000.0),
        constant_integral: entries["controller_values"]["constant_integral"]
            .clone()
            .try_into()
            .unwrap_or(0.02),
        constant_proportional: entries["controller_values"]["constant_proportional"]
            .clone()
            .try_into()
            .unwrap_or(400.0),
        target_temperature: entries["controller_values"]["target_temperature"]
            .clone()
            .try_into()
            .unwrap_or(65),
        update_interval: entries["inout"]["update_interval"]
            .clone()
            .try_into()
            .unwrap_or(3.0),
        output_path: PathBuf::from(
            entries["inout"]["output_path"]
                .clone()
                .try_into()
                .unwrap_or("/run/macpifan/values"),
        ),
        output_integral: entries["inout"]["output_integral"]
            .clone()
            .try_into()
            .unwrap_or(true),
        output_temperature: entries["inout"]["output_temperature"]
            .clone()
            .try_into()
            .unwrap_or(true),
        output_speed: entries["inout"]["output_speed"]
            .clone()
            .try_into()
            .unwrap_or(true),
    };

    println!("starting macpifan");
    println!("| i_0\t| k_i\t| k_p\t| target\t|");
    println!(
        "| {:.1e}\t| {}\t| {}\t| {}\t\t|",
        config.initial_integral,
        config.constant_proportional,
        config.constant_integral,
        config.target_temperature
    );

    return config;
}

fn take_fan_control(path: &PathBuf) {
    let mut file = File::options()
        .write(true)
        .open(path.join("fan1_manual"))
        .expect("couldnt open fan1_manual, are you root?");

    file.write("1".as_bytes()).expect("fucked up the write");
}

fn read_speed(path: &PathBuf) -> usize {
    fs::read_to_string(path.join("fan1_input"))
        .unwrap()
        .trim()
        .parse::<usize>()
        .unwrap()
}

fn write_speed(path: &PathBuf, mut speed: usize) {
    take_fan_control(path);
    speed = speed.min(6200).max(1300);
    let mut file = File::options()
        .write(true)
        .read(true)
        .open(path.join("fan1_output"))
        .unwrap();
    file.write_all(format!("{}", speed).as_bytes()).unwrap();
    let mut contents = vec![0_u8; 100];
    file.read_to_end(&mut contents).unwrap();
}

fn read_max_temperature(path: &PathBuf) -> i64 {
    let mut acc = 0;
    for i in 6..=13 {
        let mut string = "".to_string();
        let mut file = File::open(path.join(format!("temp{}_input", i))).unwrap();
        file.read_to_string(&mut string).unwrap();
        string = string.trim_ascii_end().to_string();
        acc += string.parse::<u64>().unwrap_or(71000) / 1000;
    }
    return (acc / 8) as i64;
}
