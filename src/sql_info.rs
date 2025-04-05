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
        // (SELECT/INSERT/UPDATE/DELETE)と余白
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
            if let Some(count) = sql_info.query_counts.get_mut(&query_type) {
                *count += 1;
            }

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
