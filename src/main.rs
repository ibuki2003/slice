use clap::Parser;
use std::{
    collections::VecDeque,
    io::{BufReader, BufWriter, ErrorKind, Read, Seek, Write as _},
    os::unix::fs::FileTypeExt,
};

#[derive(Debug)]
enum SliceIdx {
    FromStart(usize),
    FromEnd(usize),
}
impl From<isize> for SliceIdx {
    fn from(i: isize) -> Self {
        if i >= 0 {
            SliceIdx::FromStart(i as usize)
        } else {
            SliceIdx::FromEnd((-i) as usize)
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Range in the format start:end, where start and end can be negative.
    /// If start is omitted, it defaults to 0. If end is omitted, it defaults to the length of the input.
    range: String,

    /// Input file. If omitted, stdin is used.
    input: Option<String>,

    /// Count by bytes instead of lines.
    #[arg(short = 'c', long = "byte")]
    byte_mode: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let range_str = &args.range;
    let (s, e) = range_str.split_once(':').ok_or("Invalid range format")?;
    let start = if s.is_empty() {
        SliceIdx::FromStart(0)
    } else {
        s.parse::<isize>()?.into()
    };
    let end = if e.is_empty() {
        SliceIdx::FromEnd(0)
    } else if let Some(v) = e.strip_prefix('+') {
        let n: usize = v.parse()?;
        match start {
            SliceIdx::FromStart(m) => SliceIdx::FromStart(m + n),
            SliceIdx::FromEnd(m) => SliceIdx::FromEnd(m - n),
        }
    } else {
        e.parse::<isize>()?.into()
    };

    let mut bufwriter = std::io::BufWriter::new(std::io::stdout());

    let mode = if args.byte_mode {
        CountModeEnum::Byte
    } else {
        CountModeEnum::Line
    };

    if args.input.as_ref().is_none_or(|s| s == "-") {
        // stdin
        let mut bufreader = std::io::BufReader::new(std::io::stdin());

        slice_stream_wrapper(start, end, &mut bufreader, &mut bufwriter, mode)?;
    } else {
        // file
        let mut file = std::fs::File::open(args.input.unwrap())?;
        let ftype = file.metadata()?.file_type();
        if ftype.is_dir() {
            return Err("Input file is a directory".into());
        }

        // let seekable = file.seek(std::io::SeekFrom::Start(0)).is_ok();
        let seekable = ftype.is_file() || ftype.is_block_device();
        if seekable && mode == CountModeEnum::Byte {
            // just use seek
            // let size = file.metadata()?.len() as isize;
            let size = file.seek(std::io::SeekFrom::End(0))? as isize;
            // eprintln!("size: {}", size);
            let start = match start {
                SliceIdx::FromStart(n) => n as isize,
                SliceIdx::FromEnd(n) => size - n as isize,
            }
            .clamp(0, size);
            let end = match end {
                SliceIdx::FromStart(n) => n as isize,
                SliceIdx::FromEnd(n) => size - n as isize,
            }
            .clamp(0, size);
            if start >= end {
                return Ok(());
            }
            file.seek(std::io::SeekFrom::Start(start as u64))?;
            let mut handle = file.take((end - start) as u64);
            std::io::copy(&mut handle, &mut bufwriter)?;
        } else {
            let mut bufreader = std::io::BufReader::new(file);
            slice_stream_wrapper(start, end, &mut bufreader, &mut bufwriter, mode)?;
        }
    }

    Ok(())
}

trait CountMode {
    fn count(c: u8) -> usize;
}

struct CountModeByte;
impl CountMode for CountModeByte {
    #[inline]
    fn count(_c: u8) -> usize {
        1
    }
}

struct CountModeLine;
impl CountMode for CountModeLine {
    #[inline]
    fn count(c: u8) -> usize {
        // NOTE: with UTF-8, comparing bytes is safe.
        (c == b'\n') as usize
    }
}

#[derive(PartialEq, Eq)]
enum CountModeEnum {
    Byte,
    Line,
}

fn slice_stream_wrapper(
    start: SliceIdx,
    end: SliceIdx,
    stream: &mut BufReader<impl std::io::Read>,
    out: &mut BufWriter<impl std::io::Write>,
    mode: CountModeEnum,
) -> Result<(), Box<dyn std::error::Error>> {
    match mode {
        CountModeEnum::Byte => slice_stream::<CountModeByte>(start, end, stream, out),
        CountModeEnum::Line => slice_stream::<CountModeLine>(start, end, stream, out),
    }
}

#[inline]
fn read_char(
    reader: &mut BufReader<impl std::io::Read>,
) -> Result<Option<u8>, Box<dyn std::error::Error>> {
    let mut c = [0; 1];
    match reader.read_exact(&mut c) {
        Ok(()) => Ok(Some(c[0])),
        Err(e) => match e.kind() {
            ErrorKind::UnexpectedEof => Ok(None),
            _ => Err(e.into()),
        },
    }
}

fn slice_stream<M: CountMode>(
    start: SliceIdx,
    end: SliceIdx,
    stream: &mut BufReader<impl std::io::Read>,
    out: &mut BufWriter<impl std::io::Write>,
) -> Result<(), Box<dyn std::error::Error>> {
    // let mut queue = VecDeque::new();

    // start
    match (start, end) {
        (SliceIdx::FromStart(n), SliceIdx::FromStart(m)) => {
            if n >= m {
                return Ok(());
            }

            let mut i = 0;
            loop {
                let c = if let Some(c) = read_char(stream)? {
                    c
                } else {
                    return Ok(());
                };

                if i >= n {
                    out.write_all(&[c])?;
                }
                i += M::count(c);
                if i >= m {
                    return Ok(());
                }
            }
        }
        (SliceIdx::FromStart(n), SliceIdx::FromEnd(m)) => {
            // skip first n
            for _ in 0..n {
                if read_char(stream)?.is_none() {
                    return Ok(());
                }
            }
            let mut q = VecDeque::new();
            let mut qn = 0; // count in q
            loop {
                let c = if let Some(c) = read_char(stream)? {
                    c
                } else {
                    return Ok(());
                };
                q.push_back(c);
                qn += M::count(c);
                while qn > m {
                    let front = q.pop_front().unwrap();
                    qn -= M::count(front);
                    out.write_all(&[front])?;
                }
            }
        }
        (SliceIdx::FromEnd(n), m) => {
            let mut i = 0;
            let mut q = VecDeque::new();
            let mut qn = 0;
            loop {
                let c = if let Some(c) = read_char(stream)? {
                    c
                } else {
                    break;
                };
                q.push_back(c);
                qn += M::count(c);
                while qn > n {
                    let front = q.pop_front().unwrap();
                    let v = M::count(front);
                    qn -= v;
                    i += v;
                }
            }
            let m = match m {
                SliceIdx::FromStart(m) => m,
                SliceIdx::FromEnd(m) => i + n - m,
            };
            while i < m {
                if let Some(c) = q.pop_front() {
                    out.write_all(&[c])?;
                    i += M::count(c);
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}
