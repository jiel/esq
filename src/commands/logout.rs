// src/commands/logout.rs
use crate::utils::*;
use std::path::PathBuf;

pub fn handle_logout_command(
    existing_config: Option<Config>,
    config_file: &PathBuf,
) -> Result<(), ESQError> {
    if let Some(mut config) = existing_config {
        if config.default.password.is_some() {
            config.default.password = None;

            // Save updated configuration
            save_config(&config, config_file)?;
            println!("Successfully logged out (password removed)");
        } else {
            println!("No active session found");
        }
    } else {
        println!("No active session found");
    }
    Ok(())
}
