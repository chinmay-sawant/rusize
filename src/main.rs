use clap::Parser;
use rusize::ReportFormat;

const DEFAULT_MIN_SIZE_MB: f64 = 500.0;
const DEFAULT_DEPTH: usize = 10;

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

    #[arg(short = 'm', long = "min-size", default_value_t = DEFAULT_MIN_SIZE_MB, value_name = "MB")]
    pub min_size: f64,

    #[arg(long, default_value_t = false)]
    pub no_sort: bool,

    #[arg(short, long, default_value_t = DEFAULT_DEPTH, value_name = "LEVELS")]
    pub depth: usize,

    #[arg(short, long, value_enum, default_value_t = ReportFormat::Csv)]
    pub format: ReportFormat,

    #[arg(short, long, value_name = "OUTPUT_PATH")]
    pub output: Option<String>,

    #[arg(long, value_name = "TEXT_REPORT_PATH", help = "Open an existing text report in the interactive HTML GUI, or generate it first when combined with scan options")]
    pub gui: Option<String>,
}

fn should_generate_report_before_gui(args: &Args) -> bool {
    args.path.is_some()
        || args.min_size != DEFAULT_MIN_SIZE_MB
        || args.no_sort
        || args.depth != DEFAULT_DEPTH
        || args.output.is_some()
        || args.format != ReportFormat::Csv
}

fn main() -> anyhow::Result<()> {
    // Enable ANSI escape code processing on Windows cmd.exe / PowerShell.
    enable_ansi_support::enable_ansi_support().ok();

    let args = Args::parse();

    if let Some(gui_path) = args.gui.clone() {
        if should_generate_report_before_gui(&args) {
            if let Some(output_path) = &args.output {
                if output_path != &gui_path {
                    return Err(anyhow::anyhow!(
                        "When using --gui with scan options, --output must match --gui or be omitted"
                    ));
                }
            }

            rusize::run(
                args.path,
                args.min_size,
                !args.no_sort,
                args.depth,
                ReportFormat::Text,
                Some(gui_path.clone()),
            )?;

            return rusize::services::gui::start(&gui_path);
        }

        return rusize::services::gui::start(&gui_path);
    }
    
    rusize::run(
        args.path,
        args.min_size,
        !args.no_sort,
        args.depth,
        args.format,
        args.output,
    )
}

#[cfg(test)]
mod tests {
    use super::{should_generate_report_before_gui, Args, DEFAULT_DEPTH, DEFAULT_MIN_SIZE_MB};
    use rusize::ReportFormat;

    fn base_args() -> Args {
        Args {
            path: None,
            min_size: DEFAULT_MIN_SIZE_MB,
            no_sort: false,
            depth: DEFAULT_DEPTH,
            format: ReportFormat::Csv,
            output: None,
            gui: Some("rusize_report.txt".to_string()),
        }
    }

    #[test]
    fn gui_only_mode_does_not_trigger_regeneration() {
        assert!(!should_generate_report_before_gui(&base_args()));
    }

    #[test]
    fn gui_with_path_triggers_regeneration() {
        let mut args = base_args();
        args.path = Some("C:/".to_string());

        assert!(should_generate_report_before_gui(&args));
    }

    #[test]
    fn gui_with_text_format_triggers_regeneration() {
        let mut args = base_args();
        args.format = ReportFormat::Text;

        assert!(should_generate_report_before_gui(&args));
    }
}
