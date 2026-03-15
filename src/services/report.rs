use crate::models::dir_node::DirNode;
use crate::utils::format_size_gb;
use clap::ValueEnum;
use serde::Serialize;
use std::fs::File;
use std::io::Write;

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum ReportFormat {
    #[default]
    Csv,
    Json,
    Text,
}

pub fn generate_report(nodes: &[DirNode], format: &ReportFormat, output_file: Option<&str>) -> anyhow::Result<()> {
    match format {
        ReportFormat::Csv => generate_csv(nodes, output_file.unwrap_or("rusize_report.csv")),
        ReportFormat::Json => generate_json(nodes, output_file.unwrap_or("rusize_report.json")),
        ReportFormat::Text => generate_text(nodes, output_file.unwrap_or("rusize_report.txt")),
    }
}

fn generate_csv(nodes: &[DirNode], output_path: &str) -> anyhow::Result<()> {
    let file = File::create(output_path)?;
    let mut wtr = csv::Writer::from_writer(file);

    #[derive(Serialize)]
    struct CsvRow<'a> {
        name: &'a str,
        path: String,
        size_gb: String,
    }

    fn write_csv_recursive<'a>(
        wtr: &mut csv::Writer<File>,
        nodes: &'a [DirNode],
    ) -> anyhow::Result<()> {
        for node in nodes {
            wtr.serialize(CsvRow {
                name: &node.name,
                path: node.path.display().to_string(),
                size_gb: format_size_gb(node.size),
            })?;
            write_csv_recursive(wtr, &node.children)?;
        }
        Ok(())
    }

    write_csv_recursive(&mut wtr, nodes)?;
    wtr.flush()?;
    println!("CSV report saved to {}", output_path);
    Ok(())
}

fn generate_json(nodes: &[DirNode], output_path: &str) -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct JsonNode<'a> {
        name: &'a str,
        path: String,
        size_gb: String,
        children: Vec<JsonNode<'a>>,
    }

    fn transform_json<'a>(nodes: &'a [DirNode]) -> Vec<JsonNode<'a>> {
        nodes.iter().map(|n| JsonNode {
            name: &n.name,
            path: n.path.display().to_string(),
            size_gb: format_size_gb(n.size),
            children: transform_json(&n.children),
        }).collect()
    }

    let json_nodes = transform_json(nodes);
    let json = serde_json::to_string_pretty(&json_nodes)?;
    
    let mut file = File::create(output_path)?;
    file.write_all(json.as_bytes())?;
    println!("JSON report saved to {}", output_path);
    Ok(())
}

fn generate_text(nodes: &[DirNode], output_path: &str) -> anyhow::Result<()> {
    let mut file = File::create(output_path)?;

    fn write_tree(nodes: &[DirNode], indent: &str, depth: usize, file: &mut File) -> anyhow::Result<()> {
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

            let formatted_size = format_size_gb(node.size);

            writeln!(file, "{}{} {} ({})", indent, prefix, node.name, formatted_size)?;

            let new_indent = if depth == 0 {
                indent.to_string()
            } else if last {
                format!("{}    ", indent)
            } else {
                format!("{}│   ", indent)
            };

            write_tree(&node.children, &new_indent, depth + 1, file)?;
        }
        Ok(())
    }

    write_tree(nodes, "", 0, &mut file)?;
    println!("Text report saved to {}", output_path);
    Ok(())
}
