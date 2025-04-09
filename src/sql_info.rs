use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryType {
    Select,
    Insert,
    Update,
    Delete,
}

pub struct SqlQueryInfo {
    pub query_counts: HashMap<QueryType, usize>,
    pub table_counts: HashMap<String, usize>,
}

impl SqlQueryInfo {
    pub fn new() -> Self {
        let mut query_counts = HashMap::new();
        query_counts.insert(QueryType::Select, 0);
        query_counts.insert(QueryType::Insert, 0);
        query_counts.insert(QueryType::Update, 0);
        query_counts.insert(QueryType::Delete, 0);

        Self {
            query_counts,
            table_counts: HashMap::new(),
        }
    }

    pub fn from_message(message: &str) -> Option<Self> {
        if message.contains("SELECT ")
            || message.contains("INSERT ")
            || message.contains("UPDATE ")
            || message.contains("DELETE ")
        {
            let logs = [message];
            Some(parse_sql_from_logs(&logs))
        } else {
            None
        }
    }

    pub fn merge(&mut self, other: &SqlQueryInfo) {
        for (query_type, count) in &other.query_counts {
            if *count > 0 {
                *self.query_counts.entry(*query_type).or_insert(0) += count;
            }
        }

        for (table_name, count) in &other.table_counts {
            *self.table_counts.entry(table_name.clone()).or_insert(0) += count;
        }
    }

    pub fn total_queries(&self) -> usize {
        self.query_counts.values().sum()
    }

    pub fn query_count(&self, query_type: QueryType) -> usize {
        *self.query_counts.get(&query_type).unwrap_or(&0)
    }

    pub fn sorted_tables(&self) -> Vec<(&String, &usize)> {
        let mut tables: Vec<_> = self.table_counts.iter().collect();
        tables.sort_by(|a, b| a.0.cmp(b.0));
        tables
    }

    pub fn display_line_count(&self) -> usize {
        self.table_counts.len() + 4
    }
}

pub fn parse_sql_from_logs(logs: &[&str]) -> SqlQueryInfo {
    let mut sql_info = SqlQueryInfo::new();

    let table_pattern = match regex::Regex::new(
        r#"(?:FROM|JOIN|UPDATE|INTO)\s+(?:"([a-zA-Z0-9_]+)"|([a-zA-Z0-9_]+))(?:\s|\)|$)"#,
    ) {
        Ok(re) => re,
        Err(e) => {
            eprintln!("Regex error: {}", e);
            regex::Regex::new(r"").unwrap()
        }
    };

    for msg in logs {
        let query_type = if msg.contains("SELECT ") {
            Some(QueryType::Select)
        } else if msg.contains("UPDATE ") {
            Some(QueryType::Update)
        } else if msg.contains("INSERT ") {
            Some(QueryType::Insert)
        } else if msg.contains("DELETE ") {
            Some(QueryType::Delete)
        } else {
            None
        };

        if let Some(query_type) = query_type {
            *sql_info.query_counts.entry(query_type).or_insert(0) += 1;
            for cap in table_pattern.captures_iter(msg) {
                let table_name = cap.get(1).or_else(|| cap.get(2)).map(|m| m.as_str());

                if let Some(table_name) = table_name {
                    *sql_info
                        .table_counts
                        .entry(table_name.to_string())
                        .or_insert(0) += 1;
                }
            }
        }
    }

    sql_info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_query_info_new() {
        let info = SqlQueryInfo::new();

        // Verify query_counts initialization
        assert_eq!(info.query_count(QueryType::Select), 0);
        assert_eq!(info.query_count(QueryType::Insert), 0);
        assert_eq!(info.query_count(QueryType::Update), 0);
        assert_eq!(info.query_count(QueryType::Delete), 0);

        // Verify empty table_counts
        assert!(info.table_counts.is_empty());

        // Verify total_queries is 0
        assert_eq!(info.total_queries(), 0);
    }

    #[test]
    fn test_sql_query_info_from_message() {
        // SELECT query
        let select_msg = "SQL (0.5ms) SELECT * FROM users WHERE id = 1";
        let info = SqlQueryInfo::from_message(select_msg).unwrap();
        assert_eq!(info.query_count(QueryType::Select), 1);
        assert_eq!(info.query_count(QueryType::Insert), 0);
        assert_eq!(info.total_queries(), 1);
        assert!(info.table_counts.contains_key("users"));

        // INSERT query
        let insert_msg = "SQL (0.8ms) INSERT INTO products (name, price) VALUES ('Test', 9.99)";
        let info = SqlQueryInfo::from_message(insert_msg).unwrap();
        assert_eq!(info.query_count(QueryType::Insert), 1);
        assert_eq!(info.total_queries(), 1);
        assert!(info.table_counts.contains_key("products"));

        // Non-SQL message
        let non_sql_msg = "Processing request";
        assert!(SqlQueryInfo::from_message(non_sql_msg).is_none());
    }

    #[test]
    fn test_sql_query_info_merge() {
        let mut info1 = SqlQueryInfo::new();
        info1.query_counts.insert(QueryType::Select, 2);
        info1.table_counts.insert("users".to_string(), 2);

        let mut info2 = SqlQueryInfo::new();
        info2.query_counts.insert(QueryType::Select, 1);
        info2.query_counts.insert(QueryType::Update, 1);
        info2.table_counts.insert("users".to_string(), 1);
        info2.table_counts.insert("orders".to_string(), 1);

        info1.merge(&info2);

        // Check merged query counts
        assert_eq!(info1.query_count(QueryType::Select), 3);
        assert_eq!(info1.query_count(QueryType::Update), 1);
        assert_eq!(info1.query_count(QueryType::Insert), 0);
        assert_eq!(info1.query_count(QueryType::Delete), 0);

        // Check merged table counts
        assert_eq!(*info1.table_counts.get("users").unwrap(), 3);
        assert_eq!(*info1.table_counts.get("orders").unwrap(), 1);

        // Check total queries
        assert_eq!(info1.total_queries(), 4);
    }

    #[test]
    fn test_sorted_tables() {
        let mut info = SqlQueryInfo::new();
        info.table_counts.insert("zebra".to_string(), 3);
        info.table_counts.insert("apple".to_string(), 1);
        info.table_counts.insert("banana".to_string(), 2);

        let sorted = info.sorted_tables();

        // Check that tables are sorted alphabetically
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].0, "apple");
        assert_eq!(sorted[1].0, "banana");
        assert_eq!(sorted[2].0, "zebra");

        // Check that counts are preserved
        assert_eq!(*sorted[0].1, 1);
        assert_eq!(*sorted[1].1, 2);
        assert_eq!(*sorted[2].1, 3);
    }

    #[test]
    fn test_display_line_count() {
        let mut info = SqlQueryInfo::new();
        assert_eq!(info.display_line_count(), 4); // Base count with no tables

        info.table_counts.insert("users".to_string(), 1);
        assert_eq!(info.display_line_count(), 5); // Base + 1 table

        info.table_counts.insert("orders".to_string(), 1);
        assert_eq!(info.display_line_count(), 6); // Base + 2 tables
    }

    #[test]
    fn test_parse_sql_from_logs() {
        let logs = [
            "SQL (0.5ms) SELECT * FROM users WHERE id = 1",
            "SQL (0.8ms) INSERT INTO products (name, price) VALUES ('Test', 9.99)",
            "SQL (0.3ms) UPDATE orders SET status = 'shipped' WHERE id = 123",
            "SQL (0.2ms) DELETE FROM cart_items WHERE user_id = 456",
            "SQL (0.4ms) SELECT o.* FROM orders o JOIN users u ON o.user_id = u.id",
            "Not an SQL query",
        ];

        let info = parse_sql_from_logs(&logs);

        // Check query counts
        assert_eq!(info.query_count(QueryType::Select), 2);
        assert_eq!(info.query_count(QueryType::Insert), 1);
        assert_eq!(info.query_count(QueryType::Update), 1);
        assert_eq!(info.query_count(QueryType::Delete), 1);
        assert_eq!(info.total_queries(), 5);

        // Check table counts
        assert!(info.table_counts.contains_key("users"));
        assert!(info.table_counts.contains_key("products"));
        assert!(info.table_counts.contains_key("orders"));
        assert!(info.table_counts.contains_key("cart_items"));

        // Check that JOIN tables are counted
        assert_eq!(*info.table_counts.get("orders").unwrap(), 2); // One from UPDATE, one from SELECT...JOIN
    }
}
