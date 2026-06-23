use std::fs;

pub fn frostdao_bin() -> &'static str {
    env!("CARGO_BIN_EXE_frostdao")
}

pub fn extract_json(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('{') && line.ends_with('}'))
        .map(str::to_string)
}

pub fn cleanup_state_prefix(prefix: &str) {
    let state_dir = ".frost_state";
    if let Ok(entries) = fs::read_dir(state_dir) {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(prefix))
            {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
}
