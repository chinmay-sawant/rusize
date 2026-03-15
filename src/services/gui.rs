use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write, BufReader, BufRead};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GuiNode {
    pub name: String,
    pub path: String,
    pub size_str: String,
    pub children: Vec<GuiNode>,
}

fn parse_report_line(line: &str) -> Option<(usize, String, String)> {
    let mut remaining = line;
    let mut depth = 0;

    loop {
        if let Some(rest) = remaining.strip_prefix("│   ") {
            depth += 1;
            remaining = rest;
            continue;
        }

        if let Some(rest) = remaining.strip_prefix("    ") {
            depth += 1;
            remaining = rest;
            continue;
        }

        break;
    }

    if let Some(rest) = remaining.strip_prefix("├──").or_else(|| remaining.strip_prefix("└──")) {
        depth += 1;
        remaining = rest;
    }

    let remaining = remaining.trim_start();
    if remaining.is_empty() {
        return None;
    }

    if let Some((name, size_with_paren)) = remaining.rsplit_once(" (") {
        if let Some(size) = size_with_paren.strip_suffix(')') {
            return Some((depth, name.trim().to_string(), size.trim().to_string()));
        }
    }

    Some((depth, remaining.to_string(), String::new()))
}

fn join_display_path(parent: &str, child: &str) -> String {
    let parent = parent.trim();
    let child = child.trim();

    if parent.is_empty() {
        return child.to_string();
    }

    if child.is_empty() {
        return parent.to_string();
    }

    if parent.ends_with('/') || parent.ends_with('\\') {
        return format!("{}{}", parent, child);
    }

    let separator = if parent.contains('\\') && !parent.contains('/') {
        '\\'
    } else {
        '/'
    };

    format!("{}{}{}", parent, separator, child)
}

fn parse_text_report(file_path: &str) -> anyhow::Result<Vec<GuiNode>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut roots = Vec::new();
    let mut stack: Vec<(usize, GuiNode)> = Vec::new();

    for line_result in reader.lines() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        let Some((depth, name, size_str)) = parse_report_line(&line) else {
            continue;
        };

        let mut new_node = GuiNode {
            name: name.clone(),
            path: String::new(),
            size_str,
            children: Vec::new(),
        };

        while let Some(&(top_depth, _)) = stack.last() {
            if top_depth >= depth {
                let (_d, n) = stack.pop().unwrap();
                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push(n);
                } else {
                    roots.push(n);
                }
            } else {
                break;
            }
        }

        if depth == 0 {
            new_node.path = name.clone();
        } else {
            if let Some((_, parent)) = stack.last() {
                new_node.path = join_display_path(&parent.path, &name);
            } else {
                new_node.path = name.clone();
            }
        }

        stack.push((depth, new_node));
    }

    while let Some((_d, n)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(n);
        } else {
            roots.push(n);
        }
    }

    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::{join_display_path, parse_text_report};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_report(contents: &str) -> String {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rusize-gui-{unique}.txt"));
        fs::write(&path, contents).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn joins_windows_style_display_paths_without_extra_spaces() {
        assert_eq!(join_display_path("C:/", "Windows"), "C:/Windows");
        assert_eq!(join_display_path("C:\\", "Windows"), "C:\\Windows");
        assert_eq!(join_display_path("/", "usr"), "/usr");
    }

    #[test]
    fn parses_nested_text_report_structure() {
        let report = concat!(
            "C:/ (155.65 GB)\n",
            "├── Windows (55.30 GB)\n",
            "│   ├── WinSxS (20.18 GB)\n",
            "│   │   ├── Temp (0.49 GB)\n",
            "│   └── Installer (15.99 GB)\n",
            "├── Users (49.21 GB)\n",
            "│   └── acer (49.02 GB)\n",
            "└── Program Files (28.11 GB)\n"
        );
        let path = write_temp_report(report);

        let roots = parse_text_report(&path).unwrap();

        fs::remove_file(&path).unwrap();

        assert_eq!(roots.len(), 1);
        let root = &roots[0];
        assert_eq!(root.name, "C:/");
        assert_eq!(root.children.len(), 3);
        assert_eq!(root.children[0].name, "Windows");
        assert_eq!(root.children[0].path, "C:/Windows");
        assert_eq!(root.children[0].children.len(), 2);
        assert_eq!(root.children[0].children[0].name, "WinSxS");
        assert_eq!(root.children[0].children[0].path, "C:/Windows/WinSxS");
        assert_eq!(root.children[0].children[0].children[0].path, "C:/Windows/WinSxS/Temp");
        assert_eq!(root.children[1].children[0].path, "C:/Users/acer");
    }

    #[test]
    fn parses_multiple_root_drives_from_one_report() {
        let report = concat!(
            "C:\\ (155.72 GB)\n",
            "├── Windows (55.32 GB)\n",
            "└── Users (49.26 GB)\n",
            "D:\\ (513.68 GB)\n",
            "├── SteamLibrary (179.16 GB)\n",
            "└── Movies (47.19 GB)\n"
        );
        let path = write_temp_report(report);

        let roots = parse_text_report(&path).unwrap();

        fs::remove_file(&path).unwrap();

        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].name, "C:\\");
        assert_eq!(roots[1].name, "D:\\");
        assert_eq!(roots[0].children.len(), 2);
        assert_eq!(roots[1].children.len(), 2);
        assert_eq!(roots[1].children[0].path, "D:\\SteamLibrary");
        assert_eq!(roots[1].children[1].path, "D:\\Movies");
    }
}

fn handle_connection(mut stream: TcpStream, html_template: &str, tree_json: &str) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_millis(500)));
    
    let mut buffer = [0; 1024];
    if stream.read(&mut buffer).is_err() {
        return;
    }

    let request = String::from_utf8_lossy(&buffer[..]);
    
    if request.starts_with("GET / ") {
        let html_content = html_template.replace("\"INJECT_TREE_DATA_HERE\"", tree_json);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n{}",
            html_content.len(),
            html_content
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    } else if request.starts_with("POST /open ") {
        if let Some(body_start) = request.find("\r\n\r\n") {
            let body = &request[body_start + 4..];
            let body_trimmed = body.trim_end_matches('\0').trim();
            if let Ok(decoded) = urlencoding::decode(body_trimmed) {
                let path_to_open = decoded.into_owned();
                let _ = open::that(&path_to_open);
            }
        }
        
        let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    } else {
        let response = "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot Found";
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    }
}

pub fn start(txt_path: &str) -> anyhow::Result<()> {
    if !Path::new(txt_path).exists() {
        return Err(anyhow::anyhow!("File not found: {}", txt_path));
    }

    println!("Parsing text report...");
    let roots = parse_text_report(txt_path)?;
    let tree_json = serde_json::to_string(&roots)?;

    let html_template = include_str!("gui.html");

    // Start server
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{}", port);

    println!("Starting interactive GUI at {}", url);
    let _ = open::that(&url);

    let html_template_val = html_template.to_string();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let html_clone = html_template_val.clone();
                let json_clone = tree_json.clone();
                std::thread::spawn(move || {
                    handle_connection(stream, &html_clone, &json_clone);
                });
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }

    Ok(())
}
