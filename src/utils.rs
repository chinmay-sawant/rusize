pub fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    let gb = mb / 1024.0;
    if gb >= 1.0 {
        format!("{:.2} GB", gb)
    } else {
        format!("{:.2} MB", mb)
    }
}
