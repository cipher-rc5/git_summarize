// file: src/models/incident.rs
// description: Incident model representing cyber attacks and thefts
// reference: Threat intelligence incident tracking

use clickhouse::Row;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatePrecision {
    Exact = 1,
    Month = 2,
    Year = 3,
    Approximate = 4,
}

impl DatePrecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            DatePrecision::Exact => "exact",
            DatePrecision::Month => "month",
            DatePrecision::Year => "year",
            DatePrecision::Approximate => "approximate",
        }
    }
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct Incident {
    pub document_id: String,
    pub title: String,
    pub date: i64, // Unix timestamp
    pub date_precision: String,
    pub victim: String,
    pub attack_vector: String,
    pub amount_usd: Option<f64>,
    pub description: String,
    pub source_file: String,
    pub extracted_at: u64,
}

impl Incident {
    pub fn new(
        title: String,
        date: (i64, DatePrecision),
        victim: String,
        attack_vector: String,
        amount_usd: Option<f64>,
        description: String,
        source_file: String,
    ) -> Self {
        let (timestamp, date_precision) = date;
        let extracted_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            document_id: String::new(),
            title,
            date: timestamp,
            date_precision: date_precision.as_str().to_string(),
            victim,
            attack_vector,
            amount_usd,
            description,
            source_file,
            extracted_at,
        }
    }

    pub fn with_document_id(mut self, document_id: String) -> Self {
        self.document_id = document_id;
        self
    }
}

pub struct IncidentBuilder {
    title: Option<String>,
    date: Option<i64>,
    date_precision: DatePrecision,
    victim: Option<String>,
    attack_vector: String,
    amount_usd: Option<f64>,
    description: String,
    source_file: String,
}

impl IncidentBuilder {
    pub fn new(source_file: String) -> Self {
        Self {
            title: None,
            date: None,
            date_precision: DatePrecision::Approximate,
            victim: None,
            attack_vector: String::from("unknown"),
            amount_usd: None,
            description: String::new(),
            source_file,
        }
    }

    pub fn title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    pub fn date(mut self, date: i64, precision: DatePrecision) -> Self {
        self.date = Some(date);
        self.date_precision = precision;
        self
    }

    pub fn victim(mut self, victim: String) -> Self {
        self.victim = Some(victim);
        self
    }

    pub fn attack_vector(mut self, attack_vector: String) -> Self {
        self.attack_vector = attack_vector;
        self
    }

    pub fn amount_usd(mut self, amount: f64) -> Self {
        self.amount_usd = Some(amount);
        self
    }

    pub fn description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn build(self) -> Option<Incident> {
        let date = (self.date?, self.date_precision);
        Some(Incident::new(
            self.title?,
            date,
            self.victim?,
            self.attack_vector,
            self.amount_usd,
            self.description,
            self.source_file,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incident_builder() {
        let incident = IncidentBuilder::new("/path/to/file.md".to_string())
            .title("Test Hack".to_string())
            .date(1609459200, DatePrecision::Exact)
            .victim("Exchange XYZ".to_string())
            .amount_usd(1000000.0)
            .build();

        assert!(incident.is_some());
        let incident = incident.unwrap();
        assert_eq!(incident.title, "Test Hack");
        assert_eq!(incident.amount_usd, Some(1000000.0));
    }
}
