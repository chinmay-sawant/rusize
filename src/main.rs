use clap::Parser;
use rusize::ReportFormat;

#[derive(Parser, Debug)]
#[command(
    name = "rusize",
    author = "Chinmay Sawant",
    version,
    about = "rusize -- High-speed Multi-threaded Disk Scanner",
    long_about = "rusize is a blazing-fast, multi-threaded disk space analyzer."
)]
pub struct Args {
    #[arg(value_name = "PATH")]
    pub path: Option<String>,

    #[arg(short = 'm', long = "min-size", default_value_t = 500.0, value_name = "MB")]
    pub min_size: f64,

    #[arg(long, default_value_t = false)]
    pub no_sort: bool,

    #[arg(short, long, default_value_t = 10, value_name = "LEVELS")]
    pub depth: usize,

    #[arg(short, long, value_enum, default_value_t = ReportFormat::Csv)]
    pub format: ReportFormat,

    #[arg(short, long, value_name = "OUTPUT_PATH")]
    pub output: Option<String>,
}

fn main() -> anyhow::Result<()> {
    // Enable ANSI escape code processing on Windows cmd.exe / PowerShell.
    enable_ansi_support::enable_ansi_support().ok();

    let args = Args::parse();
    
    rusize::run(
        args.path,
        args.min_size,
        !args.no_sort,
        args.depth,
        args.format,
        args.output,
    )
}
