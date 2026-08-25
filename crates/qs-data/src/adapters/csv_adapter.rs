use crate::{DataSource, RawDataAdapter, RawRecord, RawSchema};
use qs_core::DataError;
use std::{fs::File, path::Path};

pub struct CsvRawDataAdapter;

impl RawDataAdapter for CsvRawDataAdapter {
    fn adapter_id(&self) -> String {
        "csv".into()
    }

    fn detect(&self, source: &DataSource) -> Result<bool, DataError> {
        if matches!(source.format_hint.as_deref(), Some("csv")) {
            return Ok(true);
        }
        Ok(source.uri.to_lowercase().ends_with(".csv"))
    }

    fn read_schema(&self, source: &DataSource) -> Result<RawSchema, DataError> {
        let path = file_path_from_uri(&source.uri);
        let mut reader = csv::Reader::from_path(path).map_err(data_err("CSV_OPEN_SCHEMA"))?;
        let headers = reader.headers().map_err(data_err("CSV_HEADERS"))?;
        Ok(RawSchema {
            fields: headers.iter().map(str::to_owned).collect(),
        })
    }

    fn read_records(
        &self,
        source: &DataSource,
    ) -> Result<Box<dyn Iterator<Item = Result<RawRecord, DataError>>>, DataError> {
        let path = file_path_from_uri(&source.uri);
        let file = File::open(path).map_err(data_err("CSV_OPEN_RECORDS"))?;
        let mut reader = csv::Reader::from_reader(file);
        let headers = reader.headers().map_err(data_err("CSV_HEADERS"))?.clone();
        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result.map_err(data_err("CSV_RECORD"))?;
            let mut map = serde_json::Map::new();
            for (idx, value) in record.iter().enumerate() {
                let Some(key) = headers.get(idx) else {
                    continue;
                };
                map.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
            }
            rows.push(Ok(RawRecord {
                fields: serde_json::Value::Object(map),
            }));
        }
        Ok(Box::new(rows.into_iter()))
    }
}

fn file_path_from_uri(uri: &str) -> &Path {
    let path = match uri.strip_prefix("file://") {
        Some(value) => value,
        None => uri,
    };
    Path::new(path)
}

fn data_err<E: std::fmt::Display>(code: &'static str) -> impl Fn(E) -> DataError {
    move |e| DataError::Message {
        code: code.into(),
        message: e.to_string(),
        retryable: true,
    }
}
