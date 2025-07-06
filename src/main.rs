use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    transactions: Vec<Transaction>,
    accounts: std::collections::HashMap<String, CurrentAccount>,
    #[serde(default = "default_currency_symbol")]
    currency_symbol: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Transaction {
    #[serde(rename = "mortgage")]
    Mortgage {
        deduction_amount: Decimal,
        deduction_day: u32,
    },
    #[serde(rename = "salary")]
    Salary {
        amount: Decimal,
        day: u32,
    },
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

        for transaction in &config.transactions {
            match transaction {
                Transaction::Mortgage { deduction_amount, deduction_day } => {
                    if next_date.day() == *deduction_day {
                        if name == "main" {
                            next_balance -= *deduction_amount;
                        }
                        if name == "mortgage" {
                            next_balance += *deduction_amount;
                        }
                    }
                }
                Transaction::Salary { amount, day } => {
                    if name == "main" && next_date.day() == *day {
                        next_balance += *amount;
                    }
                }
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
            transactions: vec![
                Transaction::Mortgage {
                    deduction_amount: dec!(123.45),
                    deduction_day: mortgage_deduction_day
                },
                Transaction::Salary {
                    amount: dec!(2000.00),
                    day: 6
                },
            ],
            accounts,
            currency_symbol: "£".to_string(),
        }
    }

    #[test]
    fn test_config_parsing() {
        let yaml = r#"
transactions:
  - type: mortgage
    deduction_amount: 123.45
    deduction_day: 1
  - type: salary
    amount: 2000.00
    day: 6
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
        balances.get_mut("main").unwrap().date = "2025-01-04".to_string();
        let next = compute_next_day_balances(&config, &balances);
        assert_eq!(next["main"].date, "2025-01-05");
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
        let config = make_config(5);
        let mut balances = make_balances();
        balances.get_mut("main").unwrap().date = "2025-01-05".to_string();
        let next = compute_next_day_balances(&config, &balances);
        assert_eq!(next["main"].date, "2025-01-06");
        assert_eq!(next["main"].balance, dec!(10000.00) + dec!(2000.00));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_and_mortgage_same_day() {
        let mut config = make_config(7);
        config.transactions.push(Transaction::Salary {
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
        config.transactions.push(Transaction::Salary {
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
transactions:
  - type: mortgage
    deduction_amount: 123.45
    deduction_day: 1
  - type: salary
    amount: 2500.00
    day: 28
accounts:
  main:
    position:
      date: "2025-01-01"
      balance: 10000.00
currency_symbol: "£"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.transactions.len(), 2);
        assert_eq!(config.currency_symbol, "£");
        let account = config.accounts.get("main").unwrap();
        assert_eq!(account.position.balance, dec!(10000.00));
        assert_eq!(account.position.date, "2025-01-01");
    }
}

