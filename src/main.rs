use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs;

const SALARY_INCOME: &str = "salary_income";

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    transactions: Vec<Transaction>,
    accounts: std::collections::HashMap<String, Decimal>,
    #[serde(default = "default_currency_symbol")]
    currency_symbol: String,
    #[serde(default = "default_start_date")]
    start_date: chrono::NaiveDate,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Transaction {
    #[serde(rename = "mortgage")]
    Mortgage {
        deduction_amount: Decimal,
        deduction_day: u32,
        from: String,
        to: String
    },
    #[serde(rename = "salary")]
    Salary {
        amount: Decimal,
        day: u32,
    },
}

fn default_currency_symbol() -> String {
    "£".to_string()
}

fn default_start_date() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
}

fn main() {

    // Yes this function can be written as pure stateless code, using for instance a fold, and Copilot can do that, but this mutable is more traditional and maintainable
    // Load config from YAML

    let yaml = fs::read_to_string("config.yaml").expect("Failed to read config.yaml");
    let config: Config = match serde_yaml::from_str(&yaml) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("YAML parsing error: {e}");
            std::process::exit(1);
        }
    };

    
    let mut date = config.start_date;
    let mut balances: std::collections::HashMap<String, Decimal> = config.accounts.clone();

    // Add salary_income account with initial balance of 0 if it is absent
    if !balances.contains_key(SALARY_INCOME) {
        balances.insert(SALARY_INCOME.to_string(), Decimal::ZERO);
    }

    let opening_balance: Decimal = balances.values().sum();
    balances.insert("opening_balances".to_string(), -opening_balance);

    for _ in 0..600 {
        date = date + chrono::Duration::days(1);
        balances = compute_next_day_balances(&config, &balances, date);

        // Print a position on last day of month
        if date.month() != (date + chrono::Duration::days(1)).month() {
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

    // For each transaction, apply its effect to the relevant accounts
    for transaction in &config.transactions {
        match transaction {
            Transaction::Mortgage { deduction_amount, deduction_day, from, to } => {
                if date.day() == *deduction_day {
                    *new_balances.get_mut(from).expect("From account not found in balances") -= *deduction_amount;
                    *new_balances.get_mut(to).expect("To account not found in balances") += *deduction_amount;
                }
            }
            Transaction::Salary { amount, day } => {
                if date.day() == *day {
                    *new_balances.get_mut("main").expect("Main account not found for salary") += *amount;
                    *new_balances.get_mut(SALARY_INCOME).expect("salary_income not found for salary") -= *amount;
                }
            }
        }
    }

    // assert balances sum to zero
    let total_balance: Decimal = new_balances.values().sum();
    if total_balance != Decimal::ZERO {
        // print all balances
        for (name, balance) in &new_balances {
            print_balance_named(name, date, *balance, &config.currency_symbol);
        }
        // print error message and exit
        eprintln!("Error: Balances do not sum to zero on {date}: {total_balance}");
        std::process::exit(1);
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
            ("main".to_string(), dec!(10000.00)),
            ("mortgage".to_string(), dec!(500000.00)),
        ]);
        Config {
            transactions: vec![
                Transaction::Mortgage {
                    deduction_amount: dec!(123.45),
                    deduction_day: mortgage_deduction_day,
                    from: "main".to_string(),
                    to: "mortgage".to_string(),
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
    from: main
    to: mortgage
  - type: salary
    amount: 2000.00
    day: 6
accounts:
  main: 10000.00
  mortgage: 500000.00

currency_symbol: "£"
start_date: "2025-01-01"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config, make_config(1));
        let account = config.accounts.get("main").unwrap();
        assert_eq!(*account, dec!(10000.00));
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
    from: main
    to: mortgage
  - type: salary
    amount: 2500.00
    day: 28
accounts:
  main: 10000.00
currency_symbol: "£"
start_date: "2025-01-01"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.transactions.len(), 2);
        assert_eq!(config.currency_symbol, "£");
        let account = config.accounts.get("main").unwrap();
        assert_eq!(*account, dec!(10000.00));
    }
}

