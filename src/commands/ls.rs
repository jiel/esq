// src/commands/ls.rs
use crate::elasticsearch::client::ElasticsearchClient;
use crate::utils::*;
use serde_json::Value;

fn display_indices(indices: &[Value]) {
    for index_data in indices {
        if let Some(index_name) = index_data["index"].as_str() {
            println!("{}", index_name);
        }
    }
}

pub fn handle_ls_command(existing_config: Option<Config>) -> Result<(), ESQError> {
    let config = existing_config
        .ok_or_else(|| {
            ESQError::ConfigError("No configuration found. Please login first.".to_string())
        })?
        .clone();

    let es = ElasticsearchClient::new(config)?;
    let indices = es.list_indices()?;
    display_indices(&indices);

    Ok(())
}
