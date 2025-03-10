// src/commands/login.rs
// Import necessary libraries
use reqwest;
use rpassword;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::utils::DefaultConfig;
use crate::utils::*;

// Structure to hold the login context
struct LoginContext {
    config: DefaultConfig,
}

impl LoginContext {
    // Create a new LoginContext with the provided configuration
    fn new(config: DefaultConfig) -> Self {
        Self { config }
    }
}

// Get the URL from the user or existing configuration
fn get_url(url: &Option<String>, existing_config: &Option<Config>) -> Result<String, ESQError> {
    let url: String = match (url, existing_config) {
        (Some(url), _) => url.clone(),
        (None, Some(config)) => {
            print!("URL [{}]: ", config.default.url);
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            if input.is_empty() {
                config.default.url.clone()
            } else {
                input.to_string()
            }
        }
        (None, None) => {
            print!("URL: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };
    Ok(url)
}

// Get the username and password from the user, using existing values as defaults
fn get_credentials(existing_config: &Option<Config>) -> Result<(String, String), ESQError> {
    // Modified username prompt to use existing value as default
    let username = if let Some(config) = existing_config {
        match config.default.username.clone() {
            Some(username) => {
                print!("Username [{}]: ", username);
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let input = input.trim();
                if input.is_empty() {
                    username
                } else {
                    input.to_string()
                }
            }
            None => {
                print!("Username: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                input.trim().to_string()
            }
        }
    } else {
        print!("Username: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    let password = rpassword::prompt_password("Password: ")?;

    Ok((username, password))
}

// Test the connection to the Elasticsearch server
fn test_connection(url: &str, config: &DefaultConfig) -> Result<bool, ESQError> {
    let client = reqwest::blocking::Client::new();
    let es_test_url = format!("{}/_cat", url.trim_end_matches('/'));

    let mut request = client.get(&es_test_url);

    if let Some(ref password) = config.password {
        if let Some(ref username) = config.username {
            request = request.basic_auth(username, Some(password));
        }
    }

    let response = request.send()?;
    if !response.status().is_success() {
        return Ok(false);
    }

    let text = response.text()?;
    if !text.contains("/_cat/") {
        return Err(ESQError::ConfigError(
            "The server doesn't appear to be an Elasticsearch instance".to_string(),
        ));
    }

    Ok(true)
}

// Function to attempt a connection to the Elasticsearch server
fn attempt_connection(
    url: &str,
    login_context: &mut LoginContext,
    config_file: &PathBuf,
) -> Result<(), ESQError> {
    if test_connection(url, &login_context.config)? {
        println!("Successfully connected to Elasticsearch!");
        println!("Credentials are temporarily stored in ~/.esq/config.toml");
        println!("Remove them after use with the 'logout' command");

        save_config(
            &Config {
                default: login_context.config.clone(),
            },
            config_file,
        )?;
        Ok(())
    } else {
        println!("Authentication failed with provided credentials.");
        Err(ESQError::AuthError)
    }
}

// Handle the login command, managing the login process
pub fn handle_login_command(
    existing_config: Option<Config>,
    config_file: &PathBuf,
) -> Result<(), ESQError> {
    // Create a login context by calling the get_url function with existing_config if it exists
    let url = get_url(&None, &existing_config)?;
    let mut login_context = LoginContext::new(DefaultConfig {
        url: url.clone(),
        username: None,
        password: None,
    });

    // If a username exists in existing_config, call the get_credentials method
    if let Some(config) = &existing_config {
        if let Some(_username) = &config.default.username {
            let (username, password) = get_credentials(&existing_config)?;
            login_context.config.username = Some(username);
            login_context.config.password = Some(password);
            // Attempt to connect with authentication
            attempt_connection(&url, &mut login_context, config_file)?;
            return Ok(());
        }
    }

    // Attempt to connect to the server without authentication
    if test_connection(
        &url,
        &DefaultConfig {
            username: None,
            password: None,
            ..Default::default()
        },
    )? {
        println!("Successfully connected to Elasticsearch!");
        save_config(
            &Config {
                default: login_context.config.clone(),
            },
            config_file,
        )?;
        Ok(())
    } else {
        // If an authentication error occurs (401 code)
        let (username, password) = get_credentials(&existing_config)?;
        login_context.config.username = Some(username);
        login_context.config.password = Some(password);
        // Attempt to connect with authentication
        attempt_connection(&url, &mut login_context, config_file)?;
        Ok(())
    }
}
