use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct Config {
    mortgage: Mortgage,
    opening: Opening,
    #[serde(default = "default_currency_symbol")]
    currency_symbol: String,
}

#[derive(Debug, Deserialize)]
struct Mortgage {
    deduction_amount: Decimal,
    deduction_day: u32,
}

#[derive(Debug, Deserialize)]
struct Opening {
    date: String,
    balance: Decimal,
}

fn default_currency_symbol() -> String {
    "£".to_string()
}

fn main() {
    // Load config from YAML
    let yaml = fs::read_to_string("config.yaml").expect("Failed to read config.yaml");
    let config: Config = serde_yaml::from_str(&yaml).expect("Failed to parse YAML");

    // Set today to 1st Jan 2025
    let today = chrono::NaiveDate::from_ymd_opt(2025, 1, 1).expect("Invalid date");

    let balance = (today, config.opening.balance);

    let mut baln = balance;

    for _ in 0..60 {
        compute_next_day_balance(&config, &mut baln);
        print_balance(baln, &config.currency_symbol);
    }
}

fn compute_next_day_balance(config: &Config, baln: &mut (chrono::NaiveDate, Decimal)) {
    *baln = (baln.0 + chrono::Duration::days(1), baln.1);
    if baln.0.day() == config.mortgage.deduction_day {
        baln.1 -= config.mortgage.deduction_amount;
    }
}

fn print_balance(balance: (chrono::NaiveDate, Decimal), currency_symbol: &str) {
    let date = balance.0;
    // Format to 2 decimal places and prefix with £
    println!("{date} {symbol}{v:.2}", date = date, symbol = currency_symbol, v = balance.1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_config(deduction_day: u32) -> Config {
        Config {
            mortgage: Mortgage {
                deduction_amount: dec!(123.45),
                deduction_day,
            },
            opening: Opening {
                date: "2025-01-01".to_string(),
                balance: dec!(10000.00),
            },
            currency_symbol: "£".to_string(),
        }
    }

    #[test]
    fn test_config_parsing() {
        let yaml = r#"
mortgage:
  deduction_amount: 123.45
  deduction_day: 1
opening:
  date: "2025-01-01"
  balance: 10000.00
currency_symbol: "£"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.mortgage.deduction_amount, dec!(123.45));
        assert_eq!(config.mortgage.deduction_day, 1);
        assert_eq!(config.opening.date, "2025-01-01");
        assert_eq!(config.opening.balance, dec!(10000.00));
        assert_eq!(config.currency_symbol, "£");
    }

    #[test]
    fn test_compute_next_day_balance_no_deduction() {
        let config = make_config(2);
        let mut baln = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            dec!(10000.00),
        );
        compute_next_day_balance(&config, &mut baln);
        assert_eq!(baln.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 2).unwrap());
        assert_eq!(baln.1, dec!(10000.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balance_with_deduction() {
        let config = make_config(3);
        let mut baln = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            dec!(10000.00),
        );
        compute_next_day_balance(&config, &mut baln);
        assert_eq!(baln.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 3).unwrap());
        assert_eq!(baln.1, dec!(10000.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balance_no_deduction_other_day() {
        let config = make_config(5);
        let mut baln = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            dec!(10000.00),
        );
        compute_next_day_balance(&config, &mut baln);
        assert_eq!(baln.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 3).unwrap());
        assert_eq!(baln.1, dec!(10000.00));
    }
}

