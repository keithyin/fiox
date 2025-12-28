use std::time::Duration;

use fiox::SequentialWriter;

use clap::{self, Parser};
use indicatif::ProgressStyle;

#[derive(Debug, Parser, Clone)]
#[command(version, about, long_about=None)]
pub struct Cli {
    #[arg(long = "fpath")]
    pub fpath: String,

    #[arg(
        long = "fsize", 
        help = "B/K/M/G/T",
        default_value = "1G",
        value_parser = parse_size_arg
    )]
    pub fsize: usize,

    #[arg(
        long = "bufSize", 
        help = "B/K/M/G/T",
        default_value = "1M",
        value_parser = parse_size_arg
    )]
    pub buf_size: usize,

    #[arg(long = "queryDepth", default_value_t = 4)]
    pub query_depth: usize,

    #[arg(long = "nJobs", default_value_t = 1)]
    pub n_jobs: usize,
}

fn parse_size_arg(s: &str) -> Result<usize, String> {
    parse_size(s).ok_or_else(|| format!("Invalid size format: {}", s))
}

fn parse_size(size_str: &str) -> Option<usize> {
    if size_str.is_empty() {
        return None;
    }

    let (num_part, unit) = size_str.split_at(
        size_str
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(size_str.len()),
    );

    let num: usize = num_part.parse().ok()?;
    let multiplier = match unit.to_uppercase().as_str() {
        "" | "B" => 1,
        "K" => 1024,
        "M" => 1024 * 1024,
        "G" => 1024 * 1024 * 1024,
        "T" => 1024 * 1024 * 1024 * 1024,
        _ => return None,
    };

    Some(num * multiplier)
}

fn one_thread_write(
    fpath: &str,
    tot_size: usize,
    buf_size: usize,
    query_depth: usize,
    pb: indicatif::ProgressBar,
) {
    pb.set_style(
        ProgressStyle::with_template(
            "{msg}\n{wide_bar} {binary_bytes}/{binary_total_bytes} \
         speed:{binary_bytes_per_sec} elapsed:{elapsed} eta:{eta}",
        )
        .unwrap(),
    );
    pb.set_message(format!("writing to {}", fpath));
    pb.enable_steady_tick(Duration::from_millis(200));
    let mut writer = SequentialWriter::new(fpath, 0, buf_size, query_depth).unwrap();
    let data = b"1234567890abcdefghijklmnopqrstuvwxyz\n";

    let now = std::time::Instant::now();
    let mut writed_nbytes = 0;

    loop {
        let cur_write_bytes = (tot_size - writed_nbytes).min(data.len());
        if cur_write_bytes == 0 {
            break;
        }
        writer.write(&data[..cur_write_bytes]).unwrap();
        pb.inc(cur_write_bytes as u64);
        writed_nbytes += cur_write_bytes;
    }
    pb.finish_with_message(format!("finished writing to {}", fpath));

    let elapsed = now.elapsed();
    let secs = elapsed.as_secs_f64();
    let bps = writed_nbytes as f64 / secs;
    if bps < 1024.0 {
        println!(
            "write {} bytes in {:.2} secs, speed: {:.2} B/s",
            writed_nbytes, secs, bps
        );
    } else if bps < 1024.0 * 1024.0 {
        println!(
            "write {} bytes in {:.2} secs, speed: {:.2} KB/s",
            writed_nbytes,
            secs,
            bps / 1024.0
        );
    } else if bps < 1024.0 * 1024.0 * 1024.0 {
        println!(
            "write {} bytes in {:.2} secs, speed: {:.2} MB/s",
            writed_nbytes,
            secs,
            bps / (1024.0 * 1024.0)
        );
    } else {
        println!(
            "write {} bytes in {:.2} secs, speed: {:.2} GB/s",
            writed_nbytes,
            secs,
            bps / (1024.0 * 1024.0 * 1024.0)
        );
    }
}

fn main() {
    let cli = Cli::parse();
    println!("cli:{:?}", cli);
    let mut all_threads = vec![];

    let multi_pb = indicatif::MultiProgress::new();
    for i in 0..cli.n_jobs {
        let pb = multi_pb.add(indicatif::ProgressBar::new(cli.fsize as u64));

        let fname = format!("{}.{}", cli.fpath, i);
        let tot_size = cli.fsize;
        let buf_size = cli.buf_size;
        let query_depth = cli.query_depth;
        let handle = std::thread::spawn(move || {
            one_thread_write(&fname, tot_size, buf_size, query_depth, pb);
        });
        all_threads.push(handle);
    }

    for handle in all_threads {
        handle.join().unwrap();
    }
}
