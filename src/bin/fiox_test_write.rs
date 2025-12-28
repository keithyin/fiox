use fiox::SequentialWriter;

use clap::{self, Parser};

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

fn one_thread_write(fpath: &str, tot_size: usize, buf_size: usize, query_depth: usize) {
    let mut writer = SequentialWriter::new(fpath, 0, buf_size, query_depth).unwrap();
    let data = b"1234567890abcdefghijklmnopqrstuvwxyz\n";

    let mut writed_nbytes = 0;
    loop {
        let cur_write_bytes = (tot_size - writed_nbytes).min(data.len());
        if cur_write_bytes == 0 {
            break;
        }
        writer.write(&data[..cur_write_bytes]).unwrap();
        writed_nbytes += cur_write_bytes;
    }
}

fn main() {
    let cli = Cli::parse();
    println!("cli:{:?}", cli);
    let mut all_threads = vec![];

    for i in 0..cli.n_jobs {
        let fname = format!("{}.{}", cli.fpath, i);
        let tot_size = cli.fsize;
        let buf_size = cli.buf_size;
        let query_depth = cli.query_depth;
        let handle = std::thread::spawn(move || {
            one_thread_write(&fname, tot_size, buf_size, query_depth);
        });
        all_threads.push(handle);
    }

    for handle in all_threads {
        handle.join().unwrap();
    }
}
