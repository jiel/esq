use crate::utils::ESQError;
use dateparser::parse;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct SearchQueryBuilder {
    sort_order: Value,
    size: u32,
    source_fields: Option<Vec<String>>,
    search_after: Option<Value>,
    query_range: Option<Value>,
    query_match: Option<Value>,
    use_pit: bool,
}

impl Default for SearchQueryBuilder {
    fn default() -> Self {
        Self {
            sort_order: json!([{"@timestamp": {"order": "asc"}}]),
            size: 1000,
            source_fields: None,
            search_after: None,
            query_range: None,
            query_match: None,
            use_pit: false,
        }
    }
}

impl SearchQueryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sort_order(mut self, sort_order: Value) -> Self {
        self.sort_order = sort_order;
        self
    }

    pub fn with_size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    pub fn with_source_fields(mut self, fields: Option<Vec<String>>) -> Self {
        self.source_fields = fields;
        self
    }

    pub fn with_search_after(mut self, search_after: Value) -> Self {
        self.search_after = Some(search_after);
        self
    }

    pub fn with_query_match(mut self, query_match: Option<Value>) -> Self {
        self.query_match = query_match;
        self
    }

    pub fn with_time_range(
        mut self,
        from: Option<&str>,
        to: Option<&str>,
        latency: &str,
    ) -> Result<Self, ESQError> {
        let mut range = json!({
            "@timestamp": {}
        });

        if let Some(from_str) = from {
            if let Ok(from_dt) = parse(from_str) {
                range["@timestamp"]["gte"] = json!(from_dt.to_rfc3339());
            } else {
                return Err(ESQError::DateParseError(format!(
                    "Invalid from date: {}",
                    from_str
                )));
            }
        }

        if let Some(to_str) = to {
            if let Ok(to_dt) = parse(to_str) {
                range["@timestamp"]["lt"] = json!(to_dt.to_rfc3339());
            } else {
                return Err(ESQError::DateParseError(format!(
                    "Invalid to date: {}",
                    to_str
                )));
            }
        } else {
            range["@timestamp"]["lt"] = json!(format!("now-{}", latency));
        }

        self.query_range = Some(json!({
            "range": range
        }));

        Ok(self)
    }

    pub fn with_pit(mut self, use_pit: bool) -> Self {
        self.use_pit = use_pit;
        if use_pit {
            self.sort_order =
                json!([{"@timestamp": {"order": "asc"}}, {"_shard_doc": {"order": "asc"}}]);
        }
        self
    }

    pub fn build(self) -> Value {
        let mut query = json!({
            "sort": self.sort_order,
            "size": self.size,
        });

        if let Some(fields) = self.source_fields {
            if fields.is_empty() {
                query["_source"] = json!(false);
            } else {
                query["_source"] = json!(fields);
            }
        }

        if let Some(search_after) = self.search_after {
            query["search_after"] = search_after;
        }

        // Combine query_range and query_match if both are present
        match (self.query_range, self.query_match) {
            (Some(range), Some(match_query)) => {
                query["query"] = json!({
                    "bool": {
                        "must": [
                            range,
                            match_query
                        ]
                    }
                });
            }
            (Some(range), None) => {
                query["query"] = range;
            }
            (None, Some(match_query)) => {
                query["query"] = match_query;
            }
            (None, None) => {}
        }

        query
    }
}
