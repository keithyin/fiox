use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use clap::Parser;
use fiox::{SequentialReader, SequentialWriter};
use indicatif::ProgressStyle;

/// Simple cp implementation based on fiox
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Recursive copy
    #[arg(short = 'r')]
    recursive: bool,

    src: PathBuf,
    dst: PathBuf,
}

fn main() {
    let args = Args::parse();
    cp_path(&args.src, &args.dst, args.recursive).unwrap();
}

fn cp_path(src: &Path, dst: &Path, recursive: bool) -> Result<()> {
    let meta = std::fs::metadata(src).context(format!("get meta error. {}", src.display()))?;

    if meta.is_file() {
        cp_file(src, dst)
    } else if meta.is_dir() {
        if !recursive {
            anyhow::bail!("cp: -r not specified; {} is a directory", src.display());
        }
        cp_dir(src, dst)
    } else {
        anyhow::bail!("cp: unsupported file type: {}", src.display());
    }
}

fn cp_file(src: &Path, dst: &Path) -> Result<()> {
    let src_meta = std::fs::metadata(src).context(format!("get meta error. {}", src.display()))?;
    if src_meta.len() == 0 {
        std::fs::File::create(dst)?; // create empty file
        return Ok(());
    }

    if dst.is_dir() || dst.to_str().unwrap().ends_with("/") {
        if !dst.exists() {
            std::fs::create_dir_all(dst)?;
        }
        let dst_file_path = dst.join(src.file_name().unwrap());
        return cp_file(src, &dst_file_path);
    }

    let mut src_file = SequentialReader::new(src.to_str().unwrap(), 0, 1024 * 1024, 4, None)?;
    let mut dst_file = SequentialWriter::new(dst.to_str().unwrap(), 0, 1024 * 1024, 4)?;

    let mut buf = vec![0u8; 4096];

    let pb = indicatif::ProgressBar::new(src_meta.len());
    pb.set_style(
        ProgressStyle::with_template(
            "{msg}\n{wide_bar} {binary_bytes}/{binary_total_bytes} \
         speed:{binary_bytes_per_sec} elapsed:{elapsed} eta:{eta}",
        )
        .unwrap(),
    );
    pb.set_message(format!("copying {} to {}", src.display(), dst.display()));
    pb.enable_steady_tick(Duration::from_millis(200));

    loop {
        let n = src_file.read2buf(&mut buf)?;
        pb.inc(n as u64);
        if n == 0 {
            break;
        }
        dst_file.write(&buf[..n])?;
    }
    pb.finish_with_message(format!(
        "finished copying {} to {}",
        src.display(),
        dst.display()
    ));

    Ok(())
}

fn cp_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    let mut rd = std::fs::read_dir(src)?;

    while let Some(Ok(entry)) = rd.next() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let meta = entry.metadata()?;
        if meta.is_dir() {
            cp_dir(&src_path, &dst_path)?;
        } else if meta.is_file() {
            cp_file(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
