use std::process::Command;
use std::{env, fs};

use futures::future::join_all;

async fn compress(from_path: String, to_path: String) {
    let mut command;

    if cfg!(target_os = "windows") {
        command = Command::new("cmd");
        command.arg("/C");
    } else {
        command = Command::new("sh");
        command.arg("-c");
    };

    let res = command
        .arg(format!("ffmpeg -i {} -vcodec libx265 -crf 28 -fpsmax 35 -tune zerolatency -preset medium {} -y", from_path, to_path))
        .output()
        .expect("failed to execute process");

    // if res is success, delete the original file
    if res.status.success() {
        println!(
            "Compressed '{}' successfully ! Deleting original file...",
            from_path
        );
        fs::remove_file(from_path).unwrap();
    } else {
        println!("Failed to compress file '{}' !", from_path);
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("Usage: {} <input_dir> <output_dir>", args[0]);
        return;
    }

    loop {
        let paths = fs::read_dir(&args[1]).unwrap();

        let mut tasks = vec![];
        for path in paths {
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

            let from_path = path_clone.to_str().unwrap();

            let to_path = format!(
                "{}/{}",
                args[2],
                path_clone.file_name().unwrap().to_str().unwrap()
            );

            println!("Compressing '{}' into '{}'", from_path, to_path);

            tasks.push(compress(from_path.to_owned(), to_path));
        }

        join_all(tasks).await;

        // sleep 5 seconds
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
