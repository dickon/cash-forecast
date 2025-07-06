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
    start_date: chrono::NaiveDate,
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
    balance: Decimal,
}

fn default_currency_symbol() -> String {
    "£".to_string()
}

fn main() {
    // Load config from YAML
    let yaml = fs::read_to_string("config.yaml").expect("Failed to read config.yaml");
    let config: Config = serde_yaml::from_str(&yaml).expect("Failed to parse YAML");

    let mut date = config.start_date;

    // Create a map of balances for each account as Decimal
    let mut balances: std::collections::HashMap<String, Decimal> = config
        .accounts
        .iter()
        .map(|(name, account)| (name.clone(), account.balance))
        .collect();

    for i in 0..600 {
        date = date + chrono::Duration::days(1);
        balances = compute_next_day_balances(&config, &balances, date);
        if i % 12 == 0 {
            for (name, balance) in balances.iter() {
                print_balance_named(name, date, *balance, &config.currency_symbol);
            }
        }
    }
}

fn compute_next_day_balances(
    config: &Config,
    balances: &std::collections::HashMap<String, Decimal>,
    date: chrono::NaiveDate,
) -> std::collections::HashMap<String, Decimal> {
    let mut new_balances = balances.clone();

    for (name, &balance) in balances {
        let mut next_balance = balance;

        for transaction in &config.transactions {
            match transaction {
                Transaction::Mortgage { deduction_amount, deduction_day } => {
                    if date.day() == *deduction_day {
                        if name == "main" {
                            next_balance -= *deduction_amount;
                        }
                        if name == "mortgage" {
                            next_balance += *deduction_amount;
                        }
                    }
                }
                Transaction::Salary { amount, day } => {
                    if name == "main" && date.day() == *day {
                        next_balance += *amount;
                    }
                }
            }
        }

        new_balances.insert(name.clone(), next_balance);
    }

    new_balances
}

fn print_balance_named(name: &str, date: chrono::NaiveDate, balance: Decimal, currency_symbol: &str) {
    println!(
        "{name}: {date} {symbol}{v:.2}",
        name = name,
        date = date.format("%Y-%m-%d"),
        symbol = currency_symbol,
        v = balance
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    fn make_balances() -> HashMap<String, Decimal> {
        HashMap::from([
            ("main".to_string(), dec!(10000.00)),
            ("mortgage".to_string(), dec!(500000.00)),
        ])
    }

    fn make_config(mortgage_deduction_day: u32) -> Config {
        let accounts = HashMap::from([
            ("main".to_string(), CurrentAccount {
                balance: dec!(10000.00),
                },
            ),
            ("mortgage".to_string(), CurrentAccount {
                balance: dec!(500000.00),
                },
            ),
        ]);
        Config {
            transactions: vec![
                Transaction::Mortgage {
                    deduction_amount: dec!(123.45),
                    deduction_day: mortgage_deduction_day,
                },
                Transaction::Salary {
                    amount: dec!(2000.00),
                    day: 6,
                },
            ],
            accounts,
            currency_symbol: "£".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
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
    balance: 10000.00
  mortgage:
    balance: 500000.00

currency_symbol: "£"
start_date: "2025-01-01"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config, make_config(1));
        let account = config.accounts.get("main").unwrap();
        assert_eq!(account.balance, dec!(10000.00));
        assert_eq!(config.currency_symbol, "£");
    }

    #[test]
    fn test_compute_next_day_balances_no_deduction() {
        let config = make_config(2);
        let balances = make_balances();
        let next = compute_next_day_balances(
            &config,
            &balances,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(),
        );
        assert_eq!(next["main"], dec!(10000.00));
    }

    #[test]
    fn test_compute_next_day_balances_with_deduction() {
        let config = make_config(3);
        let balances = make_balances();
        let next = compute_next_day_balances(
            &config,
            &balances,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 3).unwrap(),
        );
        assert_eq!(next["main"], dec!(10000.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary() {
        let config = make_config(5);
        let balances = make_balances();
        let next = compute_next_day_balances(
            &config,
            &balances,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
        );
        assert_eq!(next["main"], dec!(10000.00) + dec!(2000.00));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_and_mortgage_same_day() {
        let mut config = make_config(7);
        config.transactions.push(Transaction::Salary {
            amount: dec!(1500.00),
            day: 7,
        });
        let mut balances = make_balances();
        balances.insert("main".to_string(), dec!(5000.00));
        let next = compute_next_day_balances(
            &config,
            &balances,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
        );
        assert_eq!(next["main"], dec!(5000.00) + dec!(1500.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_not_on_salary_day() {
        let mut config = make_config(10);
        config.transactions.push(Transaction::Salary {
            amount: dec!(1000.00),
            day: 15,
        });
        let mut balances = make_balances();
        balances.insert("main".to_string(), dec!(8000.00));
        let next = compute_next_day_balances(
            &config,
            &balances,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        );
        assert_eq!(next["main"], dec!(8000.00) + dec!(1000.00));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_none() {
        let config = make_config(20);
        let mut balances = make_balances();
        balances.insert("main".to_string(), dec!(9000.00));
        let next = compute_next_day_balances(
            &config,
            &balances,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 20).unwrap(),
        );
        assert_eq!(next["main"], dec!(9000.00) - dec!(123.45));
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
    balance: 10000.00
currency_symbol: "£"
start_date: "2025-01-01"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.transactions.len(), 2);
        assert_eq!(config.currency_symbol, "£");
        let account = config.accounts.get("main").unwrap();
        assert_eq!(account.balance, dec!(10000.00));
    }
}

