use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    mortgage: Mortgage,
    accounts: std::collections::HashMap<String, CurrentAccount>,
    #[serde(default = "default_currency_symbol")]
    currency_symbol: String,
    #[serde(default)]
    salary: Option<Salary>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Mortgage {
    deduction_amount: Decimal,
    deduction_day: u32
}

#[derive(Debug, Deserialize, PartialEq)]
struct CurrentAccount {
    position: Position,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
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

    // Create a map of balances for each account as Position
    let mut balances: std::collections::HashMap<String, Position> = config
        .accounts
        .iter()
        .map(|(name, account)| (name.clone(), account.position.clone()))
        .collect();

    for _ in 0..60 {
        balances = compute_next_day_balances(&config, &balances);
        for (name, position) in balances.iter() {
            print_balance_named(name, position, &config.currency_symbol);
        }
    }
}



fn compute_next_day_balances(
    config: &Config,
    balances: &std::collections::HashMap<String, Position>,
) -> std::collections::HashMap<String, Position> {
    let mut new_balances = balances.clone();

    for (name, position) in balances {
        let current_date = chrono::NaiveDate::parse_from_str(&position.date, "%Y-%m-%d")
            .expect("Invalid date in position");
        let next_date = current_date + chrono::Duration::days(1);
        let mut next_balance = position.balance;

        if name == "main" {
            if let Some(salary) = &config.salary {
                if next_date.day() == salary.day {
                    next_balance += salary.amount;
                }
            }
        }

        if next_date.day() == config.mortgage.deduction_day {
            if name == "main" {
                next_balance -= config.mortgage.deduction_amount;
            }
            if name == "mortgage" {
                next_balance += config.mortgage.deduction_amount;
            }            
        }

        new_balances.insert(
            name.clone(),
            Position {
                date: next_date.format("%Y-%m-%d").to_string(),
                balance: next_balance,
            },
        );
    }

    new_balances
}

fn print_balance(balance: (chrono::NaiveDate, Decimal), currency_symbol: &str) {
    let date = balance.0;
    // Format to 2 decimal places and prefix with the configured currency symbol
    println!("{date} {symbol}{v:.2}", date = date, symbol = currency_symbol, v = balance.1);
}

fn print_balance_named(name: &str, position: &Position, currency_symbol: &str) {
    println!(
        "{name}: {date} {symbol}{v:.2}",
        name = name,
        date = position.date,
        symbol = currency_symbol,
        v = position.balance
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    fn make_balances() -> HashMap<String, Position> {
        let mut balances = HashMap::new();
        balances.insert(
            "main".to_string(),
            Position {
                date: "2025-01-01".to_string(),
                balance: dec!(10000.00),
            },
        );
        balances.insert(
            "mortgage".to_string(),
            Position {
                date: "2025-01-01".to_string(),
                balance: dec!(500000.00),
            },
        );
        balances
    }

    fn make_config(mortgage_deduction_day: u32) -> Config {
        let mut accounts = HashMap::new();
        accounts.insert(
            "main".to_string(),
            CurrentAccount {
                position: Position {
                    date: "2025-01-01".to_string(),
                    balance: dec!(10000.00),
                },
            },
        );
        accounts.insert(
            "mortgage".to_string(),
            CurrentAccount {
                position: Position {
                    date: "2025-01-01".to_string(),
                    balance: dec!(500000.00),
                },
            },
        );
        Config {
            mortgage: Mortgage {
                deduction_amount: dec!(123.45),
                deduction_day: mortgage_deduction_day
            },
            accounts,
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
accounts:
  main:
    position:
      date: "2025-01-01"
      balance: 10000.00
  mortgage:
    position:
      date: "2025-01-01"
      balance: 500000.00

currency_symbol: "£"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config, make_config(1));
        let account = config.accounts.get("main").unwrap();
        assert_eq!(account.position.date, "2025-01-01");
        assert_eq!(account.position.balance, dec!(10000.00));
        assert_eq!(config.currency_symbol, "£");
    }

    #[test]
    fn test_compute_next_day_balances_no_deduction() {
        let config = make_config(2);
        let mut balances = make_balances();
        balances.get_mut("main").unwrap().date = "2025-01-05".to_string();
        let next = compute_next_day_balances(&config, &balances);
        assert_eq!(next["main"].date, "2025-01-06");
        assert_eq!(next["main"].balance, dec!(10000.00));
    }

    #[test]
    fn test_compute_next_day_balances_with_deduction() {
        let config = make_config(3);
        let mut balances = make_balances();
        balances.get_mut("main").unwrap().date = "2025-01-02".to_string();
        let next = compute_next_day_balances(&config, &balances);
        assert_eq!(next["main"].date, "2025-01-03");
        assert_eq!(next["main"].balance, dec!(10000.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary() {
        let mut config = make_config(5);
        config.salary = Some(Salary {
            amount: dec!(2000.00),
            day: 6,
        });
        let mut balances = make_balances();
        balances.get_mut("main").unwrap().date = "2025-01-05".to_string();
        let next = compute_next_day_balances(&config, &balances);
        assert_eq!(next["main"].date, "2025-01-06");
        assert_eq!(next["main"].balance, dec!(10000.00) + dec!(2000.00));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_and_mortgage_same_day() {
        let mut config = make_config(7);
        config.salary = Some(Salary {
            amount: dec!(1500.00),
            day: 7,
        });
        let mut balances = make_balances();
        balances.get_mut("main").unwrap().date = "2025-01-06".to_string();
        balances.get_mut("main").unwrap().balance = dec!(5000.00);
        let next = compute_next_day_balances(&config, &balances);
        assert_eq!(next["main"].date, "2025-01-07");
        assert_eq!(next["main"].balance, dec!(5000.00) + dec!(1500.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_not_on_salary_day() {
        let mut config = make_config(10);
        config.salary = Some(Salary {
            amount: dec!(1000.00),
            day: 15,
        });
        let mut balances = make_balances();
        balances.get_mut("main").unwrap().date = "2025-01-14".to_string();
        balances.get_mut("main").unwrap().balance = dec!(8000.00);
        let next = compute_next_day_balances(&config, &balances);
        assert_eq!(next["main"].date, "2025-01-15");
        assert_eq!(next["main"].balance, dec!(8000.00) + dec!(1000.00));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_none() {
        let config = make_config(20);
        let mut balances = make_balances();
        balances.get_mut("main").unwrap().date = "2025-01-19".to_string();
        balances.get_mut("main").unwrap().balance = dec!(9000.00);
        let next = compute_next_day_balances(&config, &balances);
        assert_eq!(next["main"].date, "2025-01-20");
        assert_eq!(next["main"].balance, dec!(9000.00) - dec!(123.45));
    }

    #[test]
    fn test_config_parsing_with_salary() {
        let yaml = r#"
mortgage:
  deduction_amount: 123.45
  deduction_day: 1
accounts:
  main:
    position:
      date: "2025-01-01"
      balance: 10000.00
currency_symbol: "£"
salary:
  amount: 2500.00
  day: 28
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.salary, Some(Salary { amount: dec!(2500.00), day: 28 }));
        assert_eq!(config.currency_symbol, "£");
        let account = config.accounts.get("main").unwrap();
        assert_eq!(account.position.balance, dec!(10000.00));
        assert_eq!(account.position.date, "2025-01-01");
        assert_eq!(config.mortgage.deduction_day, 1);
        assert_eq!(config.mortgage.deduction_amount, dec!(123.45));
        assert_eq!(config.salary.as_ref().unwrap().amount, dec!(2500.00));
        assert_eq!(config.salary.as_ref().unwrap().day, 28);
    }
}

