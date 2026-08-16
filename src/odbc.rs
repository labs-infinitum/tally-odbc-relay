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
    connect(dsn).map(|_| ())
}

pub fn execute_sql(dsn: &str, sql: &str) -> Result<QueryResponse, OdbcError> {
    execute_sql_impl(dsn, sql)
}

#[cfg(windows)]
fn connect(dsn: &str) -> Result<odbc_api::Connection<'static>, OdbcError> {
    let env = environment()?;
    env.connect(dsn, "", "", odbc_api::ConnectionOptions::default())
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
    use odbc_api::{buffers::TextRowSet, Cursor, ResultSetMetadata};

    const BATCH_SIZE: usize = 256;

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

    let mut buffers =
        TextRowSet::for_cursor(BATCH_SIZE, &mut cursor, Some(4096)).map_err(odbc_err)?;
    let mut row_set = cursor.bind_buffer(&mut buffers).map_err(odbc_err)?;
    let mut rows = Vec::new();

    while let Some(batch) = row_set.fetch().map_err(odbc_err)? {
        for row_index in 0..batch.num_rows() {
            let row = (0..batch.num_cols())
                .map(|col_index| {
                    std::string::String::from_utf8_lossy(
                        batch.at(col_index, row_index).unwrap_or(&[]),
                    )
                    .into_owned()
                })
                .collect();
            rows.push(row);
        }
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
