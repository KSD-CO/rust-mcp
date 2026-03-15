# ClickHouse Plugin - Real Database Integration

Production-ready ClickHouse plugin for analytics and reporting.

## Features

✅ **Real ClickHouse HTTP Interface**
- Execute SQL queries
- Get table schemas and statistics
- Generate analytics reports
- List database tables
- Insert data

✅ **Production Patterns**
- HTTP interface (no native driver needed)
- URL-based authentication
- Multiple output formats (JSON, CSV, TabSeparated)
- Error handling
- Query result parsing

## Setup

### Option 1: Docker (Easiest)

```bash
# Start ClickHouse
docker run -d \
  --name clickhouse \
  -p 8123:8123 \
  -p 9000:9000 \
  clickhouse/clickhouse-server

# Wait for startup
sleep 5

# Test connection
curl http://localhost:8123/ping
# Should return: Ok.
```

### Option 2: Local Installation

Install from: https://clickhouse.com/docs/en/install

### Option 3: Cloud

Use ClickHouse Cloud: https://clickhouse.cloud/

## Environment Variables

```bash
export CLICKHOUSE_URL="http://localhost:8123"
export CLICKHOUSE_USER="default"
export CLICKHOUSE_PASSWORD=""  # Empty for default setup
export CLICKHOUSE_DATABASE="default"
```

## Run the Plugin

```bash
cargo run --example plugin_clickhouse --features plugin,plugin-native
```

## Usage Examples

### 1. Execute SQL Query

```json
{
  "tool": "clickhouse_query",
  "arguments": {
    "sql": "SELECT version()",
    "format": "TabSeparated"
  }
}
```

Response:
```
✅ Query executed successfully

Format: TabSeparated

Results:
24.3.1.2672
```

### 2. Get Table Info

```json
{
  "tool": "clickhouse_table_info",
  "arguments": {
    "table_name": "system.query_log"
  }
}
```

Response:
```
📊 Table: system.query_log
Rows: 1,234

Schema:
name    String
type    String
...
```

### 3. Generate Analytics Report

```json
{
  "tool": "clickhouse_analytics",
  "arguments": {
    "table_name": "events",
    "time_range": "7 DAY",
    "user_column": "user_id",
    "value_column": "amount"
  }
}
```

Response:
```
📈 Analytics Report
Table: events
Time Range: Last 7 DAY

2024-03-20 15:00: 1,234 events, 567 users, avg 45.67
2024-03-20 14:00: 987 events, 432 users, avg 52.34
...
```

### 4. Predefined Reports

```json
{
  "tool": "clickhouse_report",
  "arguments": {
    "report_type": "daily_summary",
    "table_name": "events"
  }
}
```

Available report types:
- `daily_summary` - Daily event counts and unique users (last 7 days)
- `hourly_activity` - Hourly breakdown (last 24 hours)
- `top_users` - Most active users (last 7 days)

### 5. List All Tables

```json
{
  "tool": "clickhouse_list_tables"
}
```

Response:
```
📊 Tables in database 'default':

• events (MergeTree) - 1,234,567 rows, 45.23 MB
• users (MergeTree) - 10,000 rows, 1.50 MB
• sessions (Log) - 500,000 rows, 12.34 MB
```

### 6. Database Statistics

```json
{
  "tool": "clickhouse_stats"
}
```

Response:
```
📊 Database Statistics: default

Tables: 15
Total Rows: 10,234,567
Total Size: 2.34 GB (2,511,627,264 bytes)
```

## Sample Data Setup

Create a test table and insert sample data:

```sql
-- Create table
CREATE TABLE events (
    timestamp DateTime,
    user_id UInt32,
    event_type String,
    value Float64
) ENGINE = MergeTree()
ORDER BY timestamp;

-- Insert sample data
INSERT INTO events VALUES
    ('2024-03-20 10:00:00', 1, 'click', 1.5),
    ('2024-03-20 10:05:00', 2, 'view', 2.3),
    ('2024-03-20 10:10:00', 1, 'purchase', 99.99);
```

Then run queries via the plugin!

## Advanced Queries

### Time-Series Analysis

```json
{
  "tool": "clickhouse_query",
  "arguments": {
    "sql": "SELECT toStartOfDay(timestamp) as day, count() as events, uniq(user_id) as users FROM events WHERE timestamp >= today() - 30 GROUP BY day ORDER BY day",
    "format": "JSONEachRow"
  }
}
```

### Aggregations

```json
{
  "tool": "clickhouse_query",
  "arguments": {
    "sql": "SELECT event_type, count() as count, avg(value) as avg_value FROM events GROUP BY event_type ORDER BY count DESC",
    "format": "JSONEachRow"
  }
}
```

### Percentiles

```json
{
  "tool": "clickhouse_query",
  "arguments": {
    "sql": "SELECT quantiles(0.5, 0.95, 0.99)(value) as percentiles FROM events",
    "format": "JSON"
  }
}
```

## Output Formats

Supported formats via `format` parameter:
- `TabSeparated` (default) - Tab-separated values
- `JSONEachRow` - One JSON object per line
- `JSON` - Single JSON array
- `CSV` - Comma-separated values
- `Pretty` - Human-readable table
- `Vertical` - Vertical format
- `Markdown` - Markdown table

## Performance Tips

1. **Use LIMIT** - Prevent large result sets
   ```sql
   SELECT * FROM large_table LIMIT 100
   ```

2. **Filter Early** - Use WHERE clauses
   ```sql
   SELECT * FROM events WHERE timestamp >= today()
   ```

3. **Aggregate** - Use GROUP BY for summaries
   ```sql
   SELECT date, count() FROM events GROUP BY date
   ```

4. **Index Usage** - Query on ORDER BY columns
   ```sql
   -- Fast (uses index):
   SELECT * FROM events WHERE timestamp > '2024-03-20'
   
   -- Slow (full scan):
   SELECT * FROM events WHERE user_id = 123
   ```

## Security Notes

⚠️ **SQL Injection Warning:**
- The plugin executes raw SQL queries
- Validate and sanitize inputs
- Use parameterized queries in production
- Limit user permissions

✅ **Best Practices:**
- Use read-only user for query tools
- Limit access to specific databases/tables
- Monitor query performance
- Set query timeouts

## Troubleshooting

**Connection refused:**
```bash
# Check if ClickHouse is running
curl http://localhost:8123/ping

# Check logs
docker logs clickhouse
```

**Authentication error:**
```bash
# Test credentials
curl -u default: http://localhost:8123/?query=SELECT%201
```

**Query timeout:**
```bash
# Increase timeout in ClickHouse config
# Or add LIMIT to queries
```

**Table not found:**
```bash
# List all tables
curl http://localhost:8123/?query=SHOW%20TABLES
```

## Example Use Cases

### 1. Daily KPI Dashboard
```json
{
  "tool": "clickhouse_report",
  "arguments": {
    "report_type": "daily_summary",
    "table_name": "user_events"
  }
}
```

### 2. Real-Time Monitoring
```json
{
  "tool": "clickhouse_query",
  "arguments": {
    "sql": "SELECT toStartOfMinute(now()) as time, count() as events FROM realtime_events WHERE timestamp >= now() - INTERVAL 5 MINUTE GROUP BY time",
    "format": "JSONEachRow"
  }
}
```

### 3. User Behavior Analysis
```json
{
  "tool": "clickhouse_report",
  "arguments": {
    "report_type": "top_users",
    "table_name": "user_actions"
  }
}
```

### 4. Custom Business Metrics
```json
{
  "tool": "clickhouse_query",
  "arguments": {
    "sql": "SELECT product_category, sum(revenue) as total_revenue, count() as sales FROM orders WHERE order_date >= today() - 30 GROUP BY product_category ORDER BY total_revenue DESC",
    "format": "Pretty"
  }
}
```

## Integration with LLMs

The ClickHouse plugin is perfect for AI-powered analytics:

1. **Natural Language to SQL** - LLM converts user questions to SQL
2. **Execute via Plugin** - Plugin runs query on real database
3. **Formatted Results** - LLM presents results in natural language

Example flow:
```
User: "How many users signed up yesterday?"
LLM: Generates SQL: "SELECT count() FROM users WHERE signup_date = yesterday()"
Plugin: Executes and returns: "1,234 users"
LLM: "1,234 users signed up yesterday"
```

## References

- [ClickHouse Documentation](https://clickhouse.com/docs)
- [HTTP Interface](https://clickhouse.com/docs/en/interfaces/http)
- [SQL Reference](https://clickhouse.com/docs/en/sql-reference)
- [Functions](https://clickhouse.com/docs/en/sql-reference/functions)

## Next Steps

1. Try with your own ClickHouse instance
2. Create custom report types
3. Add data visualization
4. Implement query caching
5. Add query templating
