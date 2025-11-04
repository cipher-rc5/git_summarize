// file: src/extractor/incident.rs
// description: incident extraction from structured markdown sections
// reference: markdown parsing and information extraction

use crate::extractor::patterns::*;
use crate::models::{DatePrecision, Incident, IncidentBuilder};
use chrono::NaiveDate;
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

pub struct IncidentExtractor;

impl IncidentExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_from_markdown(&self, content: &str, file_path: &str) -> Vec<Incident> {
        let mut incidents = Vec::new();
        let parser = Parser::new(content);

        let mut current_section = String::new();
        let mut current_title = None;
        let mut in_header = false;

        for event in parser {
            match event {
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2 | HeadingLevel::H3,
                    ..
                }) => {
                    in_header = true;
                    if !current_section.is_empty() {
                        if let Some(incident) =
                            self.parse_section(&current_section, &current_title, file_path)
                        {
                            incidents.push(incident);
                        }
                        current_section.clear();
                    }
                }
                Event::End(TagEnd::Heading { .. }) => {
                    in_header = false;
                }
                Event::Text(text) => {
                    if in_header {
                        current_title = Some(text.to_string());
                    } else {
                        current_section.push_str(&text);
                        current_section.push('\n');
                    }
                }
                _ => {}
            }
        }

        // Process final section
        if !current_section.is_empty()
            && let Some(incident) = self.parse_section(&current_section, &current_title, file_path)
        {
            incidents.push(incident);
        }

        incidents
    }

    fn parse_section(
        &self,
        section: &str,
        title: &Option<String>,
        file_path: &str,
    ) -> Option<Incident> {
        let mut builder = IncidentBuilder::new(file_path.to_string());

        if let Some(title) = title {
            builder = builder.title(title.clone());
        }

        // Extract date
        if let Some(date) = self.extract_date(section) {
            builder = builder.date(date.0, date.1);
        }

        // Extract victim
        if let Some(victim) = self.extract_victim(section) {
            builder = builder.victim(victim);
        }

        // Extract amount
        if let Some(amount) = self.extract_amount(section) {
            builder = builder.amount_usd(amount);
        }

        // Extract attack vector
        if let Some(vector) = self.extract_attack_vector(section) {
            builder = builder.attack_vector(vector);
        }

        builder = builder.description(section.chars().take(500).collect());

        builder.build()
    }

    fn extract_date(&self, text: &str) -> Option<(i64, DatePrecision)> {
        // Try ISO date format first
        if let Some(captures) = ISO_DATE.captures(text) {
            let year: i32 = captures.get(1)?.as_str().parse().ok()?;
            let month: u32 = captures.get(2)?.as_str().parse().ok()?;
            let day: u32 = captures.get(3)?.as_str().parse().ok()?;

            let date = NaiveDate::from_ymd_opt(year, month, day)?;
            let timestamp = date.and_hms_opt(0, 0, 0)?.and_utc().timestamp();
            return Some((timestamp, DatePrecision::Exact));
        }

        // Try Month Year format
        if let Some(captures) = MONTH_YEAR.captures(text) {
            let month_str = captures.get(1)?.as_str();
            let year: i32 = captures.get(2)?.as_str().parse().ok()?;
            let month = self.month_to_number(month_str)?;

            let date = NaiveDate::from_ymd_opt(year, month, 1)?;
            let timestamp = date.and_hms_opt(0, 0, 0)?.and_utc().timestamp();
            return Some((timestamp, DatePrecision::Month));
        }

        None
    }

    fn month_to_number(&self, month: &str) -> Option<u32> {
        match month {
            "January" => Some(1),
            "February" => Some(2),
            "March" => Some(3),
            "April" => Some(4),
            "May" => Some(5),
            "June" => Some(6),
            "July" => Some(7),
            "August" => Some(8),
            "September" => Some(9),
            "October" => Some(10),
            "November" => Some(11),
            "December" => Some(12),
            _ => None,
        }
    }

    fn extract_victim(&self, text: &str) -> Option<String> {
        // Look for common victim indicators
        let victim_patterns = [
            r"(?i)victim:\s*([^\n]+)",
            r"(?i)target:\s*([^\n]+)",
            r"(?i)affected:\s*([^\n]+)",
            r"(?i)exchange:\s*([^\n]+)",
        ];

        for pattern in &victim_patterns {
            if let Ok(re) = regex::Regex::new(pattern)
                && let Some(captures) = re.captures(text)
            {
                return Some(captures.get(1)?.as_str().trim().to_string());
            }
        }

        None
    }

    fn extract_amount(&self, text: &str) -> Option<f64> {
        if let Some(captures) = AMOUNT_USD.captures(text) {
            let amount_str = captures.get(1)?.as_str().replace(',', "");
            let mut amount: f64 = amount_str.parse().ok()?;

            // Check for multiplier
            if text.contains("million") || text.contains('M') {
                amount *= 1_000_000.0;
            } else if text.contains("billion") || text.contains('B') {
                amount *= 1_000_000_000.0;
            } else if text.contains("thousand") || text.contains('K') {
                amount *= 1_000.0;
            }

            return Some(amount);
        }

        None
    }

    fn extract_attack_vector(&self, text: &str) -> Option<String> {
        let vectors = [
            "phishing",
            "malware",
            "supply chain",
            "social engineering",
            "vulnerability exploit",
            "insider threat",
            "ransomware",
        ];

        for vector in &vectors {
            if text.to_lowercase().contains(vector) {
                return Some(vector.to_string());
            }
        }

        None
    }
}

impl Default for IncidentExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_extraction_iso() {
        let extractor = IncidentExtractor::new();
        let text = "The attack occurred on 2021-12-15.";
        let date = extractor.extract_date(text);

        assert!(date.is_some());
        let (_timestamp, precision) = date.unwrap();
        assert_eq!(precision, DatePrecision::Exact);
    }

    #[test]
    fn test_amount_extraction() {
        let extractor = IncidentExtractor::new();
        let text = "The hackers stole $100 million from the exchange.";
        let amount = extractor.extract_amount(text);

        assert_eq!(amount, Some(100_000_000.0));
    }

    #[test]
    fn test_victim_extraction() {
        let extractor = IncidentExtractor::new();
        let text = "Victim: Ronin Network\nDate: 2022-03-23";
        let victim = extractor.extract_victim(text);

        assert_eq!(victim, Some("Ronin Network".to_string()));
    }
}
