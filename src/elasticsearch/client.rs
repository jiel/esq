use crate::utils::*;
use serde_json::Value;
use serde_json::json;

pub struct ElasticsearchClient {
    client: reqwest::blocking::Client,
    config: Config,
    index: Option<String>,
    pit_id: Option<String>,
}

impl Drop for ElasticsearchClient {
    fn drop(&mut self) {
        if let Err(e) = self.delete_pit() {
            eprintln!("Erreur lors de la suppression du PIT: {}", e);
        }
    }
}

impl ElasticsearchClient {
    pub fn new(config: Config) -> Result<Self, ESQError> {
        let client = reqwest::blocking::Client::builder().build()?;
        Ok(Self {
            client,
            config,
            index: None,
            pit_id: None,
        })
    }

    pub fn set_index(&mut self, index: &str) {
        self.index = Some(index.to_string());
    }

    pub fn create_pit(&mut self) -> Result<(), ESQError> {
        let pit_response = add_auth(
            self.client.post(format!(
                "{}/{}/_pit?keep_alive=1m",
                self.config.default.url,
                self.index.as_ref().unwrap()
            )),
            &self.config,
        )
        .send()?
        .json::<Value>()?;

        self.pit_id = Some(
            pit_response["id"]
                .as_str()
                .ok_or_else(|| ESQError::ESError("Invalid PIT response".to_string()))?
                .to_string(),
        );
        Ok(())
    }

    pub fn delete_pit(&mut self) -> Result<(), ESQError> {
        if let Some(pit_id) = &self.pit_id {
            add_auth(
                self.client
                    .delete(format!("{}/_pit", self.config.default.url))
                    .json(&json!({"id": pit_id})),
                &self.config,
            )
            .send()?;
            self.pit_id = None;
        }
        Ok(())
    }

    pub fn search(&self, query: &Value) -> Result<Value, ESQError> {
        let mut final_query = query.clone();

        // Inject PIT if available
        if let Some(pit_id) = &self.pit_id {
            final_query["pit"] = json!({
                "id": pit_id,
                "keep_alive": "1m"
            });
        }

        let url = if self.pit_id.is_some() {
            format!("{}/_search", self.config.default.url)
        } else {
            format!(
                "{}/{}/_search",
                self.config.default.url,
                self.index.as_ref().unwrap()
            )
        };

        let response = add_auth(self.client.post(url).json(&final_query), &self.config)
            .send()?
            .json::<Value>()?;

        Ok(response)
    }

    pub fn list_indices(&self) -> Result<Vec<Value>, ESQError> {
        let url = format!(
            "{}/_cat/indices?format=json",
            self.config.default.url.trim_end_matches('/')
        );

        let response = add_auth(self.client.get(&url), &self.config).send()?;

        if !response.status().is_success() {
            return Err(ESQError::NetworkError(format!(
                "Failed to list indices. Status code: {}",
                response.status()
            )));
        }

        response
            .json()
            .map_err(|e| ESQError::ParseError(format!("Failed to parse indices: {}", e)))
    }
}
