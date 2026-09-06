use std::{
    fs,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    thread,
    time::Duration,
};

const DEFAULT_LINES: usize = 120;
const FOLLOW_POLL_INTERVAL: Duration = Duration::from_millis(500);

pub fn command(path: &Path, args: &[String]) -> Result<(), String> {
    let mut follow = false;
    let mut lines = DEFAULT_LINES;
    let mut index = 0usize;

    while index < args.len() {
        match args[index].as_str() {
            "--follow" | "-f" => {
                follow = true;
                index += 1;
            }
            "--lines" | "-n" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--lines requires a positive integer".to_owned())?;
                lines = value
                    .parse::<usize>()
                    .map_err(|_| "--lines requires a positive integer".to_owned())?;
                if lines == 0 || lines > 10_000 {
                    return Err("--lines must be between 1 and 10000".into());
                }
                index += 2;
            }
            other => return Err(format!("unknown logs option `{other}`")),
        }
    }

    if !path.is_file() {
        return Err(format!(
            "runtime log does not exist yet: {}. Start the background assistant first.",
            path.display()
        ));
    }

    print_tail(path, lines)?;
    if follow {
        println!("--- following {} (Ctrl+C to stop) ---", path.display());
        follow_file(path)?;
    }
    Ok(())
}

fn print_tail(path: &Path, lines: usize) -> Result<(), String> {
    // The runtime writer bounds the active file to roughly 5 MiB, so reading the
    // current file once is intentionally simpler and safer than an unbounded
    // reverse-seek implementation.
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read runtime log {}: {error}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let all = text.lines().collect::<Vec<_>>();
    let start = all.len().saturating_sub(lines);
    for line in &all[start..] {
        println!("{line}");
    }
    Ok(())
}

fn follow_file(path: &Path) -> Result<(), String> {
    let mut file = File::open(path)
        .map_err(|error| format!("cannot open runtime log {}: {error}", path.display()))?;
    let mut position = file
        .metadata()
        .map_err(|error| format!("cannot inspect runtime log: {error}"))?
        .len();
    file.seek(SeekFrom::Start(position))
        .map_err(|error| format!("cannot seek runtime log: {error}"))?;

    loop {
        thread::sleep(FOLLOW_POLL_INTERVAL);

        let length = match fs::metadata(path) {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("cannot inspect runtime log: {error}")),
        };

        // Rotation truncates the active file after copying it to assistant.log.1.
        // Reopen from offset zero when the active file becomes shorter.
        if length < position {
            file = File::open(path)
                .map_err(|error| format!("cannot reopen rotated runtime log: {error}"))?;
            position = 0;
        }

        if length <= position {
            continue;
        }

        file.seek(SeekFrom::Start(position))
            .map_err(|error| format!("cannot seek runtime log: {error}"))?;
        let mut chunk = Vec::with_capacity((length - position).min(256 * 1024) as usize);
        file.read_to_end(&mut chunk)
            .map_err(|error| format!("cannot follow runtime log: {error}"))?;
        position = file
            .stream_position()
            .map_err(|error| format!("cannot read runtime log position: {error}"))?;

        print!("{}", String::from_utf8_lossy(&chunk));
        std::io::stdout()
            .flush()
            .map_err(|error| format!("cannot flush followed log output: {error}"))?;
    }
}
