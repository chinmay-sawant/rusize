use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write, BufReader, BufRead};
use std::net::{TcpListener, TcpStream};
use std::path::{PathBuf, Path};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GuiNode {
    pub name: String,
    pub path: String,
    pub size_str: String,
    pub children: Vec<GuiNode>,
}

fn parse_text_report(file_path: &str) -> anyhow::Result<Vec<GuiNode>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut nodes_by_depth: Vec<Vec<GuiNode>> = Vec::new();
    // Pre-allocate enough levels to handle decent depth to avoid out-of-bounds
    for _ in 0..100 {
        nodes_by_depth.push(Vec::new());
    }

    for line_result in reader.lines() {
        let line = line_result?;
        if line.trim().is_empty() {
            continue;
        }

        let chars: Vec<char> = line.chars().collect();
        let mut depth = 0;
        let mut idx = 0;

        while idx < chars.len() {
            if chars.len() - idx >= 4 {
                let chunk: String = chars[idx..idx + 4].iter().collect();
                if chunk == "    " || chunk == "│   " {
                    depth += 1;
                    idx += 4;
                    continue;
                } else if chunk == "├── " || chunk == "└── " {
                    depth += 1;
                    idx += 4;
                    break;
                }
            }
            break;
        }

        if depth == 0 && chars.first() == Some(&' ') {
            idx = 1;
        }

        let name_start = idx;
        let mut size_str = String::new();
        let mut name = String::new();

        // Find the last " (" and ")"
        if let Some(paren_start) = line.rfind(" (") {
            if let Some(paren_end) = line.rfind(")") {
                if paren_start < paren_end {
                    // Because `chars` and byte indices don't mix directly and we just want to safely extract from `line`:
                    // We can match using `name_start` in chars to byte index.
                    let name_start_byte_idx = line.char_indices().nth(name_start).map(|(i, _)| i).unwrap_or(0);
                    name = line[name_start_byte_idx..paren_start].to_string();
                    size_str = line[paren_start + 2..paren_end].to_string();
                }
            }
        }

        if name.is_empty() {
            name = line[idx..].to_string();
        }

        let mut new_node = GuiNode {
            name: name.clone(),
            path: "".to_string(), // We will reconstruct this next
            size_str: size_str.clone(),
            children: Vec::new(),
        };

        if depth == 0 {
            nodes_by_depth[0].push(new_node);
        } else {
            let parent_depth = depth - 1;
            let parent_path = {
                if let Some(parent) = nodes_by_depth[parent_depth].last() {
                    if parent.path.is_empty() {
                        parent.name.clone()
                    } else {
                        parent.path.clone()
                    }
                } else {
                    String::new()
                }
            };
            
            if !parent_path.is_empty() {
                let mut path_buf = PathBuf::from(parent_path);
                path_buf.push(&name);
                new_node.path = path_buf.to_string_lossy().to_string();
                
                if let Some(parent) = nodes_by_depth[parent_depth].last_mut() {
                    parent.children.push(new_node.clone());
                }
            }
            nodes_by_depth[depth].push(new_node);
        }
    }

    // Since we copied children, we only need to return the roots
    let roots = nodes_by_depth[0].clone();
    
    // Fix root paths
    let mut final_roots = roots;
    for root in final_roots.iter_mut() {
        if root.path.is_empty() {
            root.path = root.name.clone();
        }
    }

    Ok(final_roots)
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
