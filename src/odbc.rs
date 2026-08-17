use crate::api::QueryResponse;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OdbcError {
    #[error("{0}")]
    Driver(String),
    #[error("tally-odbc-relay only executes ODBC on Windows")]
    UnsupportedPlatform,
}

impl OdbcError {
    pub fn message(&self) -> String {
        self.to_string()
    }
}

pub fn ping_dsn(dsn: &str) -> Result<(), OdbcError> {
    catch_odbc(|| connect(dsn).map(|_| ()))
}

pub fn execute_sql(dsn: &str, sql: &str) -> Result<QueryResponse, OdbcError> {
    catch_odbc(|| execute_sql_impl(dsn, sql))
}

fn catch_odbc<T>(f: impl FnOnce() -> Result<T, OdbcError>) -> Result<T, OdbcError> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => Err(OdbcError::Driver(panic_message(payload))),
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        format!("ODBC driver panic: {msg}")
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        format!("ODBC driver panic: {msg}")
    } else {
        "ODBC driver panic".into()
    }
}

#[cfg(windows)]
fn connect(dsn: &str) -> Result<odbc_api::Connection<'static>, OdbcError> {
    let env = environment()?;
    // Tally's driver answers SQLDriverConnect (DSN=...;) but fails SQLConnect
    // with an empty diagnostic, which is what Environment::connect uses.
    let conn_str = if dsn.contains('=') {
        dsn.to_string()
    } else {
        format!("DSN={dsn};")
    };
    env.connect_with_connection_string(&conn_str, odbc_api::ConnectionOptions::default())
        .map_err(odbc_err)
}

#[cfg(windows)]
fn environment() -> Result<&'static odbc_api::Environment, OdbcError> {
    use std::sync::OnceLock;

    static ENV: OnceLock<odbc_api::Environment> = OnceLock::new();
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let env = odbc_api::Environment::new().map_err(odbc_err)?;
    Ok(ENV.get_or_init(|| env))
}

#[cfg(windows)]
fn execute_sql_impl(dsn: &str, sql: &str) -> Result<QueryResponse, OdbcError> {
    use odbc_api::{Cursor, ResultSetMetadata};

    let conn = connect(dsn)?;
    let executed = conn.execute(sql, (), None).map_err(odbc_err)?;
    let Some(mut cursor) = executed else {
        return Ok(QueryResponse {
            columns: Vec::new(),
            rows: Vec::new(),
        });
    };

    let columns = cursor
        .column_names()
        .map_err(odbc_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| OdbcError::Driver(err.to_string()))?;
    let col_count =
        u16::try_from(columns.len()).map_err(|err| OdbcError::Driver(err.to_string()))?;

    // Row-by-row SQLGetData (wide). Tally's 64-bit driver reports 32-bit SQLLEN
    // in bound buffers, which panics odbc-api's TextRowSet.
    let mut rows = Vec::new();
    let mut text_buf = Vec::new();
    while let Some(mut row) = cursor.next_row().map_err(odbc_err)? {
        let mut values = Vec::with_capacity(columns.len());
        for col in 1..=col_count {
            text_buf.clear();
            let present = row.get_wide_text(col, &mut text_buf).map_err(odbc_err)?;
            values.push(if present {
                String::from_utf16_lossy(&text_buf)
            } else {
                String::new()
            });
        }
        rows.push(values);
    }

    Ok(QueryResponse { columns, rows })
}

#[cfg(windows)]
fn odbc_err(err: odbc_api::Error) -> OdbcError {
    OdbcError::Driver(err.to_string())
}

#[cfg(not(windows))]
fn connect(_dsn: &str) -> Result<(), OdbcError> {
    Err(OdbcError::UnsupportedPlatform)
}

#[cfg(not(windows))]
fn execute_sql_impl(_dsn: &str, _sql: &str) -> Result<QueryResponse, OdbcError> {
    Err(OdbcError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::OdbcError;

    #[test]
    fn unsupported_message_is_stable() {
        assert_eq!(
            OdbcError::UnsupportedPlatform.message(),
            "tally-odbc-relay only executes ODBC on Windows"
        );
    }
}
