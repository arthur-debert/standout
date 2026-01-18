//! Structured data serialization for dispatch.
//!
//! Handles JSON, YAML, XML, and CSV serialization of handler output.
//! These bypass template rendering entirely.

use crate::OutputMode;
use serde::Serialize;
use thiserror::Error;

/// Errors that can occur during serialization.
#[derive(Debug, Error)]
pub enum SerializeError {
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML serialization failed: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("XML serialization failed: {0}")]
    Xml(#[from] quick_xml::DeError),

    #[error("CSV serialization failed: {0}")]
    Csv(String),

    #[error("Not a structured output mode")]
    NotStructured,
}

/// Serializes data to the specified structured format.
///
/// Returns an error if the output mode is not a structured mode.
pub fn serialize_structured<T: Serialize>(
    data: &T,
    mode: OutputMode,
) -> Result<String, SerializeError> {
    match mode {
        OutputMode::Json => Ok(serde_json::to_string_pretty(data)?),
        OutputMode::Yaml => Ok(serde_yaml::to_string(data)?),
        OutputMode::Xml => Ok(quick_xml::se::to_string(data)?),
        OutputMode::Csv => serialize_csv(data),
        _ => Err(SerializeError::NotStructured),
    }
}

/// Serializes data to JSON format.
pub fn to_json<T: Serialize>(data: &T) -> Result<String, SerializeError> {
    Ok(serde_json::to_string_pretty(data)?)
}

/// Serializes data to YAML format.
pub fn to_yaml<T: Serialize>(data: &T) -> Result<String, SerializeError> {
    Ok(serde_yaml::to_string(data)?)
}

/// Serializes data to XML format.
pub fn to_xml<T: Serialize>(data: &T) -> Result<String, SerializeError> {
    Ok(quick_xml::se::to_string(data)?)
}

/// Serializes data to CSV format.
///
/// The data is first converted to JSON, then flattened for CSV output.
pub fn serialize_csv<T: Serialize>(data: &T) -> Result<String, SerializeError> {
    let json_value = serde_json::to_value(data)?;
    flatten_json_to_csv(&json_value)
}

/// Flattens JSON data to CSV format.
fn flatten_json_to_csv(value: &serde_json::Value) -> Result<String, SerializeError> {
    use serde_json::Value;

    let mut wtr = csv::Writer::from_writer(vec![]);

    match value {
        Value::Array(arr) if !arr.is_empty() => {
            // Get headers from first object
            if let Some(Value::Object(first)) = arr.first() {
                let headers: Vec<&str> = first.keys().map(|s| s.as_str()).collect();
                wtr.write_record(&headers)
                    .map_err(|e| SerializeError::Csv(e.to_string()))?;

                // Write each row
                for item in arr {
                    if let Value::Object(obj) = item {
                        let row: Vec<String> = headers
                            .iter()
                            .map(|h| obj.get(*h).map(value_to_string).unwrap_or_default())
                            .collect();
                        wtr.write_record(&row)
                            .map_err(|e| SerializeError::Csv(e.to_string()))?;
                    }
                }
            } else {
                // Array of non-objects
                wtr.write_record(["value"])
                    .map_err(|e| SerializeError::Csv(e.to_string()))?;
                for item in arr {
                    wtr.write_record(&[value_to_string(item)])
                        .map_err(|e| SerializeError::Csv(e.to_string()))?;
                }
            }
        }
        Value::Object(obj) => {
            // Single object: write as key,value pairs
            wtr.write_record(["key", "value"])
                .map_err(|e| SerializeError::Csv(e.to_string()))?;
            for (k, v) in obj {
                wtr.write_record([k.as_str(), &value_to_string(v)])
                    .map_err(|e| SerializeError::Csv(e.to_string()))?;
            }
        }
        _ => {
            // Scalar: single value
            wtr.write_record(["value"])
                .map_err(|e| SerializeError::Csv(e.to_string()))?;
            wtr.write_record(&[value_to_string(value)])
                .map_err(|e| SerializeError::Csv(e.to_string()))?;
        }
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| SerializeError::Csv(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| SerializeError::Csv(e.to_string()))
}

/// Converts a JSON value to a string for CSV output.
fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::json;

    #[derive(Serialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_to_json() {
        let data = TestData {
            name: "test".into(),
            value: 42,
        };
        let result = to_json(&data).unwrap();
        assert!(result.contains("\"name\": \"test\""));
        assert!(result.contains("\"value\": 42"));
    }

    #[test]
    fn test_to_yaml() {
        let data = TestData {
            name: "test".into(),
            value: 42,
        };
        let result = to_yaml(&data).unwrap();
        assert!(result.contains("name: test"));
        assert!(result.contains("value: 42"));
    }

    #[test]
    fn test_to_xml() {
        let data = TestData {
            name: "test".into(),
            value: 42,
        };
        let result = to_xml(&data).unwrap();
        assert!(result.contains("<name>test</name>"));
        assert!(result.contains("<value>42</value>"));
    }

    #[test]
    fn test_serialize_structured_json() {
        let data = json!({"key": "value"});
        let result = serialize_structured(&data, OutputMode::Json).unwrap();
        assert!(result.contains("\"key\": \"value\""));
    }

    #[test]
    fn test_serialize_structured_not_structured() {
        let data = json!({"key": "value"});
        let result = serialize_structured(&data, OutputMode::Term);
        assert!(matches!(result, Err(SerializeError::NotStructured)));
    }

    #[test]
    fn test_csv_array_of_objects() {
        let data = json!([
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25}
        ]);
        let result = serialize_csv(&data).unwrap();
        // Header order depends on JSON key iteration (not guaranteed)
        // Check both headers exist and data is present
        assert!(result.contains("name"));
        assert!(result.contains("age"));
        assert!(result.contains("Alice"));
        assert!(result.contains("Bob"));
        assert!(result.contains("30"));
        assert!(result.contains("25"));
    }

    #[test]
    fn test_csv_single_object() {
        let data = json!({"name": "Alice", "age": 30});
        let result = serialize_csv(&data).unwrap();
        assert!(result.contains("key,value"));
        assert!(result.contains("name,Alice"));
        assert!(result.contains("age,30"));
    }
}
