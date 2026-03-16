//! Database information resource.

use mcp_kit::prelude::*;
use mcp_kit::ReadResourceRequest;

use crate::domain::port::SharedDatabase;

/// Register the `clickhouse://database/{name}` resource on the builder.
pub fn register_database_resource(
    builder: McpServerBuilder,
    db: SharedDatabase,
) -> McpServerBuilder {
    let uri = format!("clickhouse://database/{}", db.database_name());

    builder.resource(
        Resource::new(&uri, "ClickHouse Database Info")
            .with_mime_type("application/json")
            .with_description("Database connection info and summary statistics"),
        {
            let db = db.clone();
            move |req: ReadResourceRequest| {
                let db = db.clone();
                async move {
                    let info = db.database_info().await.unwrap_or_else(|_| {
                        crate::domain::model::DatabaseInfo {
                            database: db.database_name().to_string(),
                            url: db.connection_url().to_string(),
                            user: String::from("(unavailable)"),
                            stats: None,
                        }
                    });

                    Ok(ReadResourceResult::text(
                        req.uri,
                        serde_json::to_string_pretty(&info).unwrap(),
                    ))
                }
            }
        },
    )
}
