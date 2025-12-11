use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, RwLock},
    thread::{self, sleep},
    time::{Duration, Instant},
};

fn main() {
    let (smc_path, i_0, k_i, k_p, target) = read_config();

    take_fan_control(&smc_path);

    let mut time = Instant::now();

    let integral_term = Arc::new(RwLock::new(i_0));

    let output_integral = integral_term.clone();
    let output_path = smc_path.clone();
    thread::spawn(move || {
        sleep(Duration::from_secs(1));
        loop {
            println!(
                "{:.2e}, {} deg, {} rpm",
                *output_integral.read().unwrap() as f32,
                read_max_temperature(&output_path),
                read_speed(&output_path)
            );
            sleep(Duration::from_secs(3));
        }
    });

    loop {
        let error = read_max_temperature(&smc_path) - target;
        let speed = read_speed(&smc_path);
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
        let setpoint = *integral_term.try_read().unwrap() * k_i + k_p * error as f64;
        write_speed(&smc_path, setpoint as usize);
        sleep(Duration::from_millis(50));
    }
}

fn read_config() -> (PathBuf, f64, f64, f64, i64) {
    let file = fs::read_to_string("config").expect("cant find the config file");

    let mut path_string = file.lines();
    let path = PathBuf::from(
        path_string
            .next()
            .expect("where is the smc (/sys/devices/platform/applesmc.768)"),
    );
    let i_0 = path_string
        .next()
        .expect("what is the initial integral value (10000)")
        .parse()
        .unwrap_or(10000.0);
    let k_i = path_string
        .next()
        .expect("what is the integral term (0.05)")
        .parse()
        .unwrap_or(0.05);
    let k_p = path_string
        .next()
        .expect("what is the proportional term (400.0)")
        .parse()
        .unwrap_or(400.0);

    let target = path_string
        .next()
        .expect("what is the target temperature (70)")
        .parse()
        .unwrap_or(70);

    println!("starting macpifan");
    println!("| i_0\t| k_i\t| k_p\t| target\t|");
    println!("| {:.1e}\t| {}\t| {}\t| {}\t\t|", i_0, k_i, k_p, target);

    return (path, i_0, k_i, k_p, target);
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
        acc += string.parse::<u64>().unwrap() / 1000;
    }
    return (acc / 8) as i64;
}
