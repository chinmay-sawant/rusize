use crate::models::dir_node::DirNode;
use crate::utils::format_size;
use clap::ValueEnum;
use serde::Serialize;
use std::io;

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum ReportFormat {
    #[default]
    Csv,
    Json,
    Text,
}

pub fn generate_report(nodes: &[DirNode], format: &ReportFormat) -> anyhow::Result<()> {
    match format {
        ReportFormat::Csv => generate_csv(nodes),
        ReportFormat::Json => generate_json(nodes),
        ReportFormat::Text => generate_text(nodes),
    }
}

fn generate_csv(nodes: &[DirNode]) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_writer(io::stdout());

    #[derive(Serialize)]
    struct CsvRow<'a> {
        name: &'a str,
        path: String,
        size: u64,
    }

    fn write_csv_recursive<'a>(
        wtr: &mut csv::Writer<io::Stdout>,
        nodes: &'a [DirNode],
    ) -> anyhow::Result<()> {
        for node in nodes {
            wtr.serialize(CsvRow {
                name: &node.name,
                path: node.path.display().to_string(),
                size: node.size,
            })?;
            write_csv_recursive(wtr, &node.children)?;
        }
        Ok(())
    }

    write_csv_recursive(&mut wtr, nodes)?;
    wtr.flush()?;
    Ok(())
}

fn generate_json(nodes: &[DirNode]) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(nodes)?;
    println!("{}", json);
    Ok(())
}

fn generate_text(nodes: &[DirNode]) -> anyhow::Result<()> {
    fn print_tree(nodes: &[DirNode], indent: &str, depth: usize) {
        let count = nodes.len();
        for (i, node) in nodes.iter().enumerate() {
            let last = i == count - 1;

            let prefix = if depth == 0 {
                ""
            } else if last {
                "└── "
            } else {
                "├── "
            };

            let formatted_size = format_size(node.size);

            println!("{}{} {} ({})", indent, prefix, node.name, formatted_size);

            let new_indent = if depth == 0 {
                indent.to_string()
            } else if last {
                format!("{}    ", indent)
            } else {
                format!("{}│   ", indent)
            };

            print_tree(&node.children, &new_indent, depth + 1);
        }
    }

    print_tree(nodes, "", 0);
    Ok(())
}
