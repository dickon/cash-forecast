use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    mortgage: Mortgage,
    opening: Position,
    #[serde(default = "default_currency_symbol")]
    currency_symbol: String,
    #[serde(default)]
    salary: Option<Salary>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Mortgage {
    deduction_amount: Decimal,
    deduction_day: u32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Position {
    date: String,
    balance: Decimal,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Salary {
    amount: Decimal,
    day: u32,
}

fn default_currency_symbol() -> String {
    "£".to_string()
}

fn main() {
    // Load config from YAML
    let yaml = fs::read_to_string("config.yaml").expect("Failed to read config.yaml");
    let config: Config = serde_yaml::from_str(&yaml).expect("Failed to parse YAML");

    // Set today to 1st Jan 2025
    let today = chrono::NaiveDate::parse_from_str(&config.opening.date, "%Y-%m-%d").expect("Invalid date in config");

    let mut balance = (today, config.opening.balance);

    for _ in 0..60 {
        balance = compute_next_day_balance(&config, balance);
        print_balance(balance, &config.currency_symbol);
    }
}

fn compute_next_day_balance(config: &Config, balance: (chrono::NaiveDate, Decimal)) -> (chrono::NaiveDate, Decimal) {
    let next_date = balance.0 + chrono::Duration::days(1);
    let mut next_balance = balance.1;

    if let Some(salary) = &config.salary {
        let salary = salary;
        if next_date.day() == salary.day {
            next_balance += salary.amount;
        }
    }
    if next_date.day() == config.mortgage.deduction_day {
        next_balance -= config.mortgage.deduction_amount;
    }
    (next_date, next_balance)
}

fn print_balance(balance: (chrono::NaiveDate, Decimal), currency_symbol: &str) {
    let date = balance.0;
    // Format to 2 decimal places and prefix with the configured currency symbol
    println!("{date} {symbol}{v:.2}", date = date, symbol = currency_symbol, v = balance.1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_config(mortgage_deduction_day: u32) -> Config {
        Config {
            mortgage: Mortgage {
                deduction_amount: dec!(123.45),
                deduction_day: mortgage_deduction_day,
            },
            opening: Position {
                date: "2025-01-01".to_string(),
                balance: dec!(10000.00),
            },
            currency_symbol: "£".to_string(),
            salary: None,
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
        assert_eq!(config, make_config(1));
        assert_eq!(config.mortgage.deduction_amount, dec!(123.45));
        assert_eq!(config.mortgage.deduction_day, 1);
        assert_eq!(config.opening.date, "2025-01-01");
        assert_eq!(config.opening.balance, dec!(10000.00));
        assert_eq!(config.currency_symbol, "£");
    }

    #[test]
    fn test_compute_next_day_balance_no_deduction() {
        let config = make_config(2);
        let balance = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(),
            dec!(10000.00),
        );
        let balance = compute_next_day_balance(&config, balance);
        assert_eq!(balance.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 6).unwrap());
        assert_eq!(balance.1, dec!(10000.00));
    }

    #[test]
    fn test_compute_next_day_balance_with_deduction() {
        let config = make_config(3);
        let balance = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            dec!(10000.00),
        );
        let balance = compute_next_day_balance(&config, balance);
        assert_eq!(balance.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 3).unwrap());
        assert_eq!(balance.1, dec!(10000.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balance_with_salary() {
        let mut config = make_config(5);
        config.salary = Some(Salary {
            amount: dec!(2000.00),
            day: 6,
        });
        let balance = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(),
            dec!(10000.00),
        );
        let balance = compute_next_day_balance(&config, balance);
        assert_eq!(balance.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 6).unwrap());
        assert_eq!(balance.1, dec!(10000.00) + dec!(2000.00));
    }

    #[test]
    fn test_compute_next_day_balance_with_salary_and_mortgage_same_day() {
        let mut config = make_config(7);
        config.salary = Some(Salary {
            amount: dec!(1500.00),
            day: 7,
        });
        let balance = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
            dec!(5000.00),
        );
        let balance = compute_next_day_balance(&config, balance);
        assert_eq!(balance.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 7).unwrap());
        // Salary is added, mortgage is deducted
        assert_eq!(balance.1, dec!(5000.00) + dec!(1500.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balance_with_salary_not_on_salary_day() {
        let mut config = make_config(10);
        config.salary = Some(Salary {
            amount: dec!(1000.00),
            day: 15,
        });
        let balance = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 14).unwrap(),
            dec!(8000.00),
        );
        let balance = compute_next_day_balance(&config, balance);
        assert_eq!(balance.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap());
        // Salary is added on the 15th
        assert_eq!(balance.1, dec!(8000.00) + dec!(1000.00));
    }

    #[test]
    fn test_compute_next_day_balance_with_salary_none() {
        let config = make_config(20);
        let balance = (
            chrono::NaiveDate::from_ymd_opt(2025, 1, 19).unwrap(),
            dec!(9000.00),
        );
        let balance = compute_next_day_balance(&config, balance);
        assert_eq!(balance.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 20).unwrap());
        // Only mortgage deduction applies
        assert_eq!(balance.1, dec!(9000.00) - dec!(123.45));
    }

    #[test]
    fn test_config_parsing_with_salary() {
        let yaml = r#"
mortgage:
  deduction_amount: 123.45
  deduction_day: 1
opening:
  date: "2025-01-01"
  balance: 10000.00
currency_symbol: "£"
salary:
  amount: 2500.00
  day: 28
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.salary.as_ref().unwrap().amount, dec!(2500.00));
        assert_eq!(config.salary.as_ref().unwrap().day, 28);
    }    

}

