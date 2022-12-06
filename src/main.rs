use lazy_static::lazy_static;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::{env, fs};
use tokio::sync::Mutex;

use futures::future::join_all;

// static mutable vector to store the file path, maximum duration and current duration, to be able to access them from the async function

lazy_static! {
    static ref FILES: Mutex<Vec<(String, usize, usize)>> = Mutex::new(vec![]);
}

async fn logger() {
    loop {
        {
            let files = FILES.lock().await;

            // find max file path length
            let mut max_path_length = 0;
            for file in files.iter() {
                if file.0.len() > max_path_length {
                    max_path_length = file.0.len();
                }
            }

            // print on the same line with \r
            print!("\r");
            for file in files.iter() {
                // println!("{} / {}", file.2, file.1);

                let progress = file.2 * 100 / file.1;

                // file path is padded with max_path_length spaces to align the progress bar
                print!("[ {} | {}% ]  ", file.0, progress);
            }
            println!();
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

async fn compress(from_path: String, to_path: String) {
    // replace \ with / for windows
    let from_path = from_path.replace("\\", "/");

    let mut command;

    if cfg!(target_os = "windows") {
        command = Command::new("cmd");
        command.arg("/C");
    } else {
        command = Command::new("sh");
        command.arg("-c");
        command.arg("cpulimit -l 50 -- ");
    };

    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    println!("Compressing '{}' into '{}'", from_path, to_path);

    let mut res = command
        .arg(format!(
            "ffmpeg -i {} -loglevel debug -vcodec libx265 -crf 28 -tune zerolatency -preset medium {} -y 2>&1",
            from_path, to_path
        ))
        .spawn()
        .expect("failed to execute process");

    let mut maximum: usize = 0; // matches /Duration: (\d+):(\d+):(\d+)/
    let mut current: usize = 0; // matches /time=(\d+):(\d+):(\d+)/

    {
        let stderr = res.stdout.as_mut().unwrap();
        let stderr_reader = BufReader::new(stderr);
        let stderr_lines = stderr_reader.lines();

        // println!("Output: ");
        for line in stderr_lines {
            // get maximum duration with regex
            let line = line.unwrap();
            if line.contains("Duration") {
                let re = regex::Regex::new(r"Duration: (\d+):(\d+):(\d+)").unwrap();
                let caps = re.captures(&line).unwrap();
                maximum = caps[1].parse::<usize>().unwrap() * 3600
                    + caps[2].parse::<usize>().unwrap() * 60
                    + caps[3].parse::<usize>().unwrap();

                // println!("Maximum duration: {}s", maximum);

                // Store the file name and maximum duration
                {
                    let mut files = FILES.lock().await;
                    let mut filename = from_path.clone();

                    // remove the path
                    filename = filename.split("/").last().unwrap().to_string();

                    files.push((filename, maximum, 0));
                }
            }

            // get current duration with regex
            if line.contains("time=") && maximum != 0 {
                // print progress
                let re = regex::Regex::new(r"time=(\d+):(\d+):(\d+)").unwrap();
                let caps = re.captures(&line).unwrap();
                current = caps[1].parse::<usize>().unwrap() * 3600
                    + caps[2].parse::<usize>().unwrap() * 60
                    + caps[3].parse::<usize>().unwrap();

                // let progress = current * 100 / maximum;
                // println!("{} | Progress: {}%", from_path, progress);

                // update current duration
                {
                    let mut files = FILES.lock().await;
                    for file in files.iter_mut() {
                        if from_path.ends_with(&file.0) {
                            file.2 = current;
                        }
                    }
                }
            }

            // println!("{:?}", line);
        }
    }

    println!();

    let res = res.wait().unwrap();

    // if res is success, delete the original file
    if res.success() {
        println!(
            "Compressed '{}' successfully ! Deleting original file...",
            from_path
        );
        // fs::remove_file(from_path).unwrap();
    } else {
        println!("Failed to compress file '{}' !", from_path);
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 5 {
        println!(
            "Usage: {} <input_dir> <output_dir> <timeout> <nb_threads>",
            args[0]
        );
        return;
    }

    // create logger task
    tokio::spawn(logger());

    loop {
        let paths = fs::read_dir(&args[1]).unwrap();

        let mut tasks = vec![];
        let mut i = 0;
        for path in paths {
            if i == args[4].parse().unwrap() {
                join_all(tasks).await;
                tasks = vec![];
                i = 0;
            }
            let path_clone = path.unwrap().path();

            if path_clone
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".mp4")
                == false
            {
                continue;
            }

            let from_path = path_clone.to_str().unwrap().to_string();

            let to_path = format!(
                "{}/{}",
                args[2],
                path_clone.file_name().unwrap().to_str().unwrap()
            );

            tasks.push(tokio::spawn(async move {
                compress(from_path.to_string(), to_path.to_string()).await;
            }));
            i += 1;
        }

        // sleep X seconds
        std::thread::sleep(std::time::Duration::from_secs(args[3].parse().unwrap()));
    }
}
