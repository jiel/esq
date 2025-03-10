use crate::elasticsearch::builder::SearchQueryBuilder;
use crate::elasticsearch::client::ElasticsearchClient;
use crate::utils::*;
use clap::Args;
use dateparser::parse;
use serde_json::Value;
use serde_json::json;
use std::cmp;
use std::fmt;
use std::thread;
use std::time::Duration;

const BATCH_SIZE: u32 = 1000;
const DEFAULT_NUMBER_OF_LINES: u32 = 10;
const MAX_NUMBER_OF_LINES: u32 = 5000;
const LATENCY: &str = "1m";

#[derive(Args)]
pub struct CatArgs {
    /// Index name or alias to query
    #[arg(value_name = "index_or_alias")]
    pub index: String,

    /// Display entries around a specific time
    #[arg(long, value_name = "datetime")]
    #[arg(short = 'a')]
    pub around: Option<String>,

    /// Number of lines to display
    #[arg(short = 'n', value_name = "number_of_lines", default_value_t = DEFAULT_NUMBER_OF_LINES)]
    pub lines: u32,

    /// Start time for filtering results
    #[arg(long, value_name = "datetime")]
    #[arg(short = 'F')]
    pub from: Option<String>,

    /// End time for filtering results
    #[arg(long, value_name = "datetime")]
    #[arg(short = 'T')]
    pub to: Option<String>,

    /// Select specific fields (comma-separated)
    #[arg(long = "select", value_name = "field1,field2,..")]
    #[arg(short = 's')]
    pub select_clause: Option<String>,

    /// Filter results with specific values in fields
    #[arg(long = "where", value_name = "field1:value1,field2:value2,..")]
    #[arg(short = 'w')]
    pub where_clause: Option<String>,

    /// Follow new entries in the index in real-time
    #[arg(long)]
    #[arg(short = 'f')]
    pub follow: bool,
}

#[derive(Debug, PartialEq)]
pub enum ParameterCombination {
    Around,
    To,
    From,
    FromTo,
    Follow,
    None,
}

impl fmt::Display for ParameterCombination {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParameterCombination::Around => write!(f, "around"),
            ParameterCombination::To => write!(f, "to"),
            ParameterCombination::From => write!(f, "from"),
            ParameterCombination::FromTo => write!(f, "from+to"),
            ParameterCombination::Follow => write!(f, "follow"),
            ParameterCombination::None => write!(f, "none"),
        }
    }
}

#[derive(Debug)]
pub struct WhereFilter {
    field: String,
    value: String,
}

pub struct ValidationResult {
    mode: ParameterCombination,
    select_fields: Option<Vec<String>>,
    where_filters: Option<Vec<WhereFilter>>,
}

fn validate_parameters(
    around: &Option<String>,
    from: &Option<String>,
    to: &Option<String>,
    lines: &u32,
    follow: bool,
    select_clause: &Option<String>,
    where_clause: &Option<String>,
) -> Result<ValidationResult, ESQError> {
    let select_fields = if let Some(select) = select_clause {
        if select.is_empty() {
            return Err(ESQError::ValidationError(
                "Select clause cannot be empty".to_string(),
            ));
        }
        let fields: Vec<String> = select
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if fields.is_empty() {
            return Err(ESQError::ValidationError(
                "Select clause must contain at least one field".to_string(),
            ));
        }
        Some(fields)
    } else {
        None
    };

    let where_filters = if let Some(where_str) = where_clause {
        if where_str.is_empty() {
            return Err(ESQError::ValidationError(
                "Where clause cannot be empty".to_string(),
            ));
        }
        let filters: Result<Vec<WhereFilter>, ESQError> = where_str
            .split(',')
            .map(|pair| {
                let parts: Vec<&str> = pair.split(':').collect();
                if parts.len() != 2 || parts[0].trim().is_empty() || parts[1].trim().is_empty() {
                    Err(ESQError::ValidationError(format!(
                        "Invalid where clause format. Expected 'field:value', got '{}'",
                        pair
                    )))
                } else {
                    Ok(WhereFilter {
                        field: parts[0].trim().to_string(),
                        value: parts[1].trim().to_string(),
                    })
                }
            })
            .collect();
        Some(filters?)
    } else {
        None
    };

    let mode = if around.is_some() {
        if from.is_some() || to.is_some() {
            return Err(ESQError::ValidationError(
                "The parameters --to and --from cannot be used at the same time as --around."
                    .to_string(),
            ));
        }
        if follow {
            return Err(ESQError::ValidationError(
                "The parameter --follow cannot be used at the same time as --around.".to_string(),
            ));
        }

        if *lines > MAX_NUMBER_OF_LINES {
            return Err(ESQError::ValidationError(format!(
                "In combination with --around, the -n parameter has a maximum value of {}.",
                MAX_NUMBER_OF_LINES
            )));
        }
        ParameterCombination::Around
    } else if to.is_some() {
        if follow {
            return Err(ESQError::ValidationError(
                "The parameter --follow cannot be used at the same time as --to.".to_string(),
            ));
        }

        if *lines > MAX_NUMBER_OF_LINES {
            return Err(ESQError::ValidationError(format!(
                "In combination with --to, the -n parameter has a maximum value of {}.",
                MAX_NUMBER_OF_LINES
            )));
        }
        if from.is_some() {
            if *lines != DEFAULT_NUMBER_OF_LINES {
                return Err(ESQError::ValidationError(
                    "You cannot use -n in combination with a full time range (--from and --to)."
                        .to_string(),
                ));
            }
            ParameterCombination::FromTo
        } else {
            ParameterCombination::To
        }
    } else if from.is_some() {
        if follow {
            return Err(ESQError::ValidationError(
                "The parameter --follow cannot be used at the same time as --from.".to_string(),
            ));
        }
        ParameterCombination::From
    } else if follow {
        ParameterCombination::Follow
    } else {
        ParameterCombination::None
    };

    Ok(ValidationResult {
        mode,
        select_fields,
        where_filters,
    })
}

#[derive(Debug)]
struct SeekOriginParameters {
    datetime: Option<String>,
    size: u32,
}

#[derive(Debug)]
struct ExtractionParameters {
    use_pit: bool,
    total_docs: u32,
    query_match: Option<Value>,
    search_after: Option<Value>,
    seek_origin: Option<SeekOriginParameters>,
    sort_order: Value,
    sleep_between_batches: bool,
}

impl ExtractionParameters {
    fn from_mode(
        validation: &ValidationResult,
        lines: &u32,
        around: &Option<String>,
        to: &Option<String>,
    ) -> Result<Self, ESQError> {
        match validation.mode {
            ParameterCombination::Around => Ok(Self {
                use_pit: true,
                total_docs: *lines,
                query_match: gen_query_match(&validation.where_filters),
                search_after: None,
                seek_origin: Some(SeekOriginParameters {
                    datetime: around.clone(),
                    size: *lines / 2,
                }),
                sort_order: json!([{"@timestamp": {"order": "asc"}}, {"_shard_doc": {"order": "asc"}}]),
                sleep_between_batches: false,
            }),
            ParameterCombination::To => Ok(Self {
                use_pit: true,
                total_docs: *lines,
                query_match: gen_query_match(&validation.where_filters),
                search_after: None,
                seek_origin: Some(SeekOriginParameters {
                    datetime: to.clone(),
                    size: *lines,
                }),
                sort_order: json!([{"@timestamp": {"order": "asc"}}, {"_shard_doc": {"order": "asc"}}]),
                sleep_between_batches: false,
            }),
            ParameterCombination::From => Ok(Self {
                use_pit: false,
                total_docs: *lines,
                query_match: gen_query_match(&validation.where_filters),
                search_after: None,
                seek_origin: None,
                sort_order: json!([{"@timestamp": {"order": "asc"}}]),
                sleep_between_batches: false,
            }),
            ParameterCombination::FromTo => Ok(Self {
                use_pit: true,
                total_docs: u32::MAX,
                query_match: gen_query_match(&validation.where_filters),
                search_after: None,
                seek_origin: None,
                sort_order: json!([{"@timestamp": {"order": "asc"}}, {"_shard_doc": {"order": "asc"}}]),
                sleep_between_batches: false,
            }),
            ParameterCombination::Follow => Ok(Self {
                use_pit: false,
                total_docs: u32::MAX,
                query_match: gen_query_match(&validation.where_filters),
                search_after: None,
                seek_origin: Some(SeekOriginParameters {
                    datetime: None,
                    size: *lines,
                }),
                sort_order: json!([{"@timestamp": {"order": "asc"}}]),
                sleep_between_batches: true,
            }),
            ParameterCombination::None => Ok(Self {
                use_pit: false,
                total_docs: *lines,
                query_match: gen_query_match(&validation.where_filters),
                search_after: None,
                seek_origin: Some(SeekOriginParameters {
                    datetime: None,
                    size: *lines,
                }),
                sort_order: json!([{"@timestamp": {"order": "asc"}}]),
                sleep_between_batches: false,
            }),
        }
    }

    fn should_stop(&self, hits_len: usize, remaining_docs: &mut u32) -> bool {
        if self.sleep_between_batches {
            return false;
        }

        if *remaining_docs != u32::MAX {
            *remaining_docs = remaining_docs.saturating_sub(hits_len as u32);
            *remaining_docs == 0
        } else {
            false
        }
    }

    fn update_search_after(&mut self, val: Option<&Value>) {
        self.search_after = val.cloned();
    }
}

fn seek_origin(es: &ElasticsearchClient, params: &ExtractionParameters) -> Option<Value> {
    let seek_params = params.seek_origin.as_ref()?;

    let mut query_builder = SearchQueryBuilder::new()
        .with_size(seek_params.size + 1)
        .with_source_fields(vec![].into()) // _source: false
        .with_pit(params.use_pit)
        .with_query_match(params.query_match.clone());

    // Set sort order based on whether we use PIT or not
    if params.use_pit {
        query_builder = query_builder.with_sort_order(
            json!([{"@timestamp": {"order": "desc"}}, {"_shard_doc": {"order": "asc"}}]),
        );
    } else {
        query_builder = query_builder.with_sort_order(json!([{"@timestamp": {"order": "desc"}}]));
    }

    if let Some(dt) = &seek_params.datetime {
        if let Ok(parsed_date) = parse(dt) {
            let parsed_date = parsed_date.to_rfc3339();
            query_builder = query_builder
                .with_time_range(None, Some(&parsed_date), LATENCY)
                .ok()?;
        } else {
            return None;
        }
    } else {
        query_builder = query_builder.with_time_range(None, None, LATENCY).ok()?;
    }

    let search_query = query_builder.build();

    es.search(&search_query).ok().and_then(|response| {
        response["hits"]["hits"]
            .as_array()
            .and_then(|hits| hits.last())
            .map(|last_hit| last_hit["sort"].clone())
    })
}

fn gen_query_match(filters: &Option<Vec<WhereFilter>>) -> Option<Value> {
    filters.as_ref().map(|filters| match filters.len() {
        0 => json!({"match_all": {}}),
        1 => json!({
            "match": {
                &filters[0].field: &filters[0].value
            }
        }),
        _ => {
            let mut bool_query = json!({
                "bool": {
                    "must": []
                }
            });

            for filter in filters {
                bool_query["bool"]["must"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({
                        "match": {
                            &filter.field: &filter.value
                        }
                    }));
            }

            bool_query
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub fn handle_cat_command(
    config: Option<Config>,
    index: &str,
    from: &Option<String>,
    to: &Option<String>,
    select_clause: &Option<String>,
    where_clause: &Option<String>,
    follow: bool,
    around: &Option<String>,
    lines: &u32,
) -> Result<(), ESQError> {
    let config = config
        .ok_or_else(|| {
            ESQError::ConfigError("No configuration found. Please login first.".to_string())
        })?
        .clone();

    let validation =
        validate_parameters(around, from, to, lines, follow, select_clause, where_clause)?;

    let mut es = ElasticsearchClient::new(config)?;
    es.set_index(index);

    let mut params = ExtractionParameters::from_mode(&validation, lines, around, to)?;

    if params.use_pit {
        es.create_pit()?;
    }

    if params.seek_origin.is_some() {
        params.update_search_after(seek_origin(&es, &params).as_ref());
    }

    let query_builder = SearchQueryBuilder::new()
        .with_sort_order(params.sort_order.clone())
        .with_pit(params.use_pit)
        .with_query_match(params.query_match.clone())
        .with_source_fields(validation.select_fields.clone())
        .with_time_range(from.as_deref(), to.as_deref(), LATENCY)?;

    let mut remaining_docs = params.total_docs;

    // Fetch results in batches
    loop {
        let current_size = if !params.sleep_between_batches {
            cmp::min(remaining_docs, BATCH_SIZE)
        } else {
            BATCH_SIZE
        };

        let mut current_builder = query_builder.clone().with_size(current_size);

        if let Some(ref last_sort) = params.search_after {
            current_builder = current_builder.with_search_after(last_sort.clone());
        }

        let search_query = current_builder.build();
        let response = es.search(&search_query)?;
        let hits = response["hits"]["hits"].as_array().unwrap();

        if hits.is_empty() && !params.sleep_between_batches {
            break;
        }

        for hit in hits {
            println!("{}", hit["_source"]);
        }

        if let Some(last_hit) = hits.last() {
            params.update_search_after(last_hit.get("sort"));
        }

        if params.should_stop(hits.len(), &mut remaining_docs) {
            break;
        }

        if params.sleep_between_batches {
            thread::sleep(Duration::from_secs(1));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_around_with_from() {
        let result = validate_parameters(
            &Some("2024-01-01".to_string()),
            &Some("2024-01-01".to_string()),
            &None,
            &10,
            false,
            &None,
            &None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_around_with_follow() {
        let result = validate_parameters(
            &Some("2024-01-01".to_string()),
            &None,
            &None,
            &10,
            true,
            &None,
            &None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_to_with_follow() {
        let result = validate_parameters(
            &None,
            &None,
            &Some("2024-01-01".to_string()),
            &10,
            true,
            &None,
            &None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_valid_params() {
        let result = validate_parameters(
            &None,
            &Some("2024-01-01".to_string()),
            &None,
            &10,
            false,
            &None,
            &None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().mode, ParameterCombination::From);
    }

    #[test]
    fn test_validate_select_clause() {
        let result = validate_parameters(
            &None,
            &None,
            &None,
            &10,
            false,
            &Some("field1,field2,field3".to_string()),
            &None,
        );
        assert!(result.is_ok());
        let validation = result.unwrap();
        assert_eq!(
            validation.select_fields,
            Some(vec![
                "field1".to_string(),
                "field2".to_string(),
                "field3".to_string()
            ])
        );
    }

    #[test]
    fn test_validate_empty_select_clause() {
        let result = validate_parameters(
            &None,
            &None,
            &None,
            &10,
            false,
            &Some("".to_string()),
            &None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_where_clause() {
        let result = validate_parameters(
            &None,
            &None,
            &None,
            &10,
            false,
            &None,
            &Some("field1:value1,field2:value2".to_string()),
        );
        assert!(result.is_ok());
        let validation = result.unwrap();
        let filters = validation.where_filters.unwrap();
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].field, "field1");
        assert_eq!(filters[0].value, "value1");
        assert_eq!(filters[1].field, "field2");
        assert_eq!(filters[1].value, "value2");
    }

    #[test]
    fn test_validate_invalid_where_clause() {
        let result = validate_parameters(
            &None,
            &None,
            &None,
            &10,
            false,
            &None,
            &Some("field1:value1,invalid_format".to_string()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_empty_where_clause() {
        let result = validate_parameters(
            &None,
            &None,
            &None,
            &10,
            false,
            &None,
            &Some("".to_string()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_combined_select_and_where() {
        let result = validate_parameters(
            &None,
            &None,
            &None,
            &10,
            false,
            &Some("field1,field2".to_string()),
            &Some("field1:value1".to_string()),
        );
        assert!(result.is_ok());
        let validation = result.unwrap();
        assert_eq!(
            validation.select_fields,
            Some(vec!["field1".to_string(), "field2".to_string()])
        );
        let filters = validation.where_filters.unwrap();
        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0].field, "field1");
        assert_eq!(filters[0].value, "value1");
    }

    #[test]
    fn test_validate_around_only() {
        let result = validate_parameters(
            &Some("2024-01-01".to_string()),
            &None,
            &None,
            &10,
            false,
            &None,
            &None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().mode, ParameterCombination::Around);
    }

    #[test]
    fn test_validate_to_only() {
        let result = validate_parameters(
            &None,
            &None,
            &Some("2024-01-01".to_string()),
            &10,
            false,
            &None,
            &None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().mode, ParameterCombination::To);
    }

    #[test]
    fn test_validate_from_only() {
        let result = validate_parameters(
            &None,
            &Some("2024-01-01".to_string()),
            &None,
            &10,
            false,
            &None,
            &None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().mode, ParameterCombination::From);
    }

    #[test]
    fn test_validate_from_to() {
        let result = validate_parameters(
            &None,
            &Some("2024-01-01".to_string()),
            &Some("2024-01-02".to_string()),
            &10,
            false,
            &None,
            &None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().mode, ParameterCombination::FromTo);
    }
    #[test]
    fn test_validate_from_to_invalid_n() {
        let result = validate_parameters(
            &None,
            &Some("2024-01-01".to_string()),
            &Some("2024-01-02".to_string()),
            &20,
            false,
            &None,
            &None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_around_invalid_n() {
        let result = validate_parameters(
            &Some("2024-01-01".to_string()),
            &None,
            &None,
            &10000,
            false,
            &None,
            &None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_to_invalid_n() {
        let result = validate_parameters(
            &None,
            &None,
            &Some("2024-01-01".to_string()),
            &10000,
            false,
            &None,
            &None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_from_n() {
        let result = validate_parameters(
            &None,
            &Some("2024-01-01".to_string()),
            &None,
            &20000,
            false,
            &None,
            &None,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().mode, ParameterCombination::From);
    }

    #[test]
    fn test_validate_none() {
        let result = validate_parameters(&None, &None, &None, &20, false, &None, &None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().mode, ParameterCombination::None);
    }

    #[test]
    fn test_gen_query_match_none() {
        let result = gen_query_match(&None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_gen_query_match_empty() {
        let filters = Some(vec![]);
        let result = gen_query_match(&filters);
        assert_eq!(result, Some(json!({"match_all": {}})));
    }

    #[test]
    fn test_gen_query_match_single() {
        let filters = Some(vec![WhereFilter {
            field: "level".to_string(),
            value: "ERROR".to_string(),
        }]);
        let result = gen_query_match(&filters);
        assert_eq!(
            result,
            Some(json!({
                "match": {
                    "level": "ERROR"
                }
            }))
        );
    }

    #[test]
    fn test_gen_query_match_multiple() {
        let filters = vec![
            WhereFilter {
                field: "kubernetes.namespace".to_string(),
                value: "production".to_string(),
            },
            WhereFilter {
                field: "level".to_string(),
                value: "WARN".to_string(),
            },
        ];
        let result = gen_query_match(&Some(filters));
        assert_eq!(
            result,
            Some(json!({
                "bool": {
                    "must": [
                        {
                            "match": {
                                "kubernetes.namespace": "production"
                            }
                        },
                        {
                            "match": {
                                "level": "WARN"
                            }
                        }
                    ]
                }
            }))
        );
    }
}
