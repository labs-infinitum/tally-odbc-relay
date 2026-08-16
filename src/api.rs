use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryResponse {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct HealthOk {
    pub ok: bool,
}

#[derive(Debug, Serialize)]
pub struct HealthErr {
    pub ok: bool,
    pub error: String,
}

/// Extract SQL from a JSON `{"sql":"..."}` body or raw `text/plain`.
pub fn parse_query_sql(content_type: Option<&str>, body: &str) -> Result<String, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("request body is empty".into());
    }

    let is_json = content_type
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/json")
        })
        .unwrap_or(false);

    if is_json {
        let request: QueryRequest =
            serde_json::from_str(trimmed).map_err(|err| format!("invalid JSON body: {err}"))?;
        let sql = request.sql.trim();
        if sql.is_empty() {
            return Err("sql is empty".into());
        }
        return Ok(sql.to_string());
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_query_sql;

    #[test]
    fn parses_json_sql() {
        let sql = parse_query_sql(
            Some("application/json; charset=utf-8"),
            r#"{"sql":"SELECT $Name FROM Ledger"}"#,
        )
        .unwrap();
        assert_eq!(sql, "SELECT $Name FROM Ledger");
    }

    #[test]
    fn rejects_empty_json_sql() {
        let err = parse_query_sql(Some("application/json"), r#"{"sql":"  "}"#).unwrap_err();
        assert_eq!(err, "sql is empty");
    }

    #[test]
    fn rejects_invalid_json() {
        let err = parse_query_sql(Some("application/json"), "not-json").unwrap_err();
        assert!(err.starts_with("invalid JSON body:"));
    }

    #[test]
    fn treats_plain_text_as_sql() {
        let sql = parse_query_sql(Some("text/plain"), "SELECT $Name FROM Company\n").unwrap();
        assert_eq!(sql, "SELECT $Name FROM Company");
    }

    #[test]
    fn treats_missing_content_type_as_raw_sql() {
        let sql = parse_query_sql(None, "SELECT $Name FROM ODBCTables").unwrap();
        assert_eq!(sql, "SELECT $Name FROM ODBCTables");
    }

    #[test]
    fn rejects_empty_body() {
        let err = parse_query_sql(Some("text/plain"), "   ").unwrap_err();
        assert_eq!(err, "request body is empty");
    }
}
