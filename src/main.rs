use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs;

const MAIN_ACCOUNT: &str = "main";
const SALARY_INCOME: &str = "salary_income";
const MORTGAGE_INCOME: &str = "mortgage_income";
const MORTGAGE_ACCOUNT: &str = "mortgage";
const OPENING_BALANCES: &str = "opening_balances";

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
        #[serde(default = "default_main")]
        from: String,
        #[serde(default = "default_mortgage")]
        to: String,
    },
    #[serde(rename = "salary")]
    Salary {
        amount: Decimal,
        day: u32,
        #[serde(default = "default_main")]
        to: String,
    },
}

fn default_currency_symbol() -> String {
    "£".to_string()
}

fn default_start_date() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
}

fn default_main() -> String {
    MAIN_ACCOUNT.to_string()
}

fn default_mortgage() -> String {
    MORTGAGE_ACCOUNT.to_string()
}

fn main() {
    // Load config from YAML
    // read from actual.yaml if it exists, otherwise from config.yaml
    let config_file = if fs::metadata("actual.yaml").is_ok() {
        "actual.yaml"
    } else {
        "config.yaml"
    };
    let yaml = fs::read_to_string(config_file).expect("Failed to read config file");
    let config: Config = match serde_yaml::from_str(&yaml) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("YAML parsing error: {e}");
            std::process::exit(1);
        }
    };

    // Work out balances before running
    let accounts_with_defaults = add_default_accounts(&config.accounts);
    let balances = add_opening_balances(&accounts_with_defaults);

    let history = run(&config, balances, 600);
    // Print the history of balances
    for (date, balances) in history {
        if date.day() == 1 {
            println!("\nBalances on {date}:");
            for (name, balance) in &balances {
                print_balance_named(name, date, *balance, &config.currency_symbol); 
            }
        }
    }
}

fn run(
    config: &Config,
    balances: std::collections::HashMap<String, Decimal>,
    days_to_run: i32
) -> Vec<(chrono::NaiveDate, std::collections::HashMap<String, Decimal>)> {
    let mut balances = balances;
    let mut date: chrono::NaiveDate = config.start_date;
    let mut history = Vec::new();

    for _ in 0..days_to_run {
        date = date + chrono::Duration::days(1);
        balances = compute_next_day_balances(config, &balances, date);
        history.push((date, balances.clone()));
    }
    history
}

fn add_opening_balances(
    balances: &std::collections::HashMap<String, Decimal>,
) -> std::collections::HashMap<String, Decimal> {
    let mut new_balances = balances.clone();
    let opening_balance: Decimal = new_balances.values().sum();
    new_balances.insert(OPENING_BALANCES.to_string(), -opening_balance);
    new_balances
}

fn add_default_accounts(
    balances: &std::collections::HashMap<String, Decimal>,
) -> std::collections::HashMap<String, Decimal> {
    let mut new_balances = balances.clone();
    if !new_balances.contains_key(SALARY_INCOME) {
        new_balances.insert(SALARY_INCOME.to_string(), Decimal::ZERO);
    }
    if !new_balances.contains_key(MORTGAGE_INCOME) {
        new_balances.insert(MORTGAGE_INCOME.to_string(), Decimal::ZERO);
    }
    new_balances
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
            Transaction::Salary { amount, day, to } => {
                if date.day() == *day {
                    *new_balances.get_mut(to).expect("Salary 'to' account not found") += *amount;
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
        panic!("Error: Balances do not sum to zero on {date}: {total_balance}");
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

    fn create_test_accounts(mortgage_deduction_day: u32) -> Config {
        let accounts = HashMap::from([
            (MAIN_ACCOUNT.to_string(), dec!(10000.00)),
            (MORTGAGE_ACCOUNT.to_string(), dec!(500000.00)),
        ]);
        let accounts_with_defaults = super::add_default_accounts(&accounts);
        let accounts_with_opening = add_opening_balances(&accounts_with_defaults);
        Config {
            transactions: vec![
                Transaction::Mortgage {
                    deduction_amount: dec!(123.45),
                    deduction_day: mortgage_deduction_day,
                    from: MAIN_ACCOUNT.to_string(),
                    to: MORTGAGE_ACCOUNT.to_string(),
                },
                Transaction::Salary {
                    amount: dec!(2000.00),
                    day: 6,
                    to: MAIN_ACCOUNT.to_string()
                },
            ],
            accounts: accounts_with_opening,
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
  main: 10000.00
  mortgage: 500000.00
currency_symbol: "£"
start_date: "2025-01-01"
"#;
        let original_config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        let mut config = original_config;
        config.accounts = add_default_accounts(&config.accounts);
        config.accounts = add_opening_balances(&config.accounts);   
        
        let expected = create_test_accounts(1);
        assert_eq!(
            config, expected,
            "Config parsed from YAML does not match expected.\nParsed: {:#?}\nExpected: {:#?}",
            config, expected
        );
        let account = config.accounts.get(MAIN_ACCOUNT).unwrap();
        assert_eq!(*account, dec!(10000.00));
        assert_eq!(config.currency_symbol, "£");
    }

    #[test]
    fn test_compute_next_day_balances_no_deduction() {
        let mortgage_deduction_day = 2;
        let test_day = 5;
        let next = make_accounts_for_day(mortgage_deduction_day, test_day);
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00));
    }

    fn make_accounts_for_day(mortgage_deduction_day: u32, test_day: u32) -> HashMap<String, Decimal> {
        let config = create_test_accounts(mortgage_deduction_day);
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, test_day).unwrap(),
        );
        next
    }
    
    #[test]
    fn test_compute_next_day_balances_with_deduction() {
        let next = make_accounts_for_day(3, 3);
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary() {
        let next= make_accounts_for_day(5, 6);
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00) + dec!(2000.00));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_and_mortgage_same_day() {
        let mut config = create_test_accounts(7);
        config.transactions.push(Transaction::Salary {
            amount: dec!(1500.00),
            day: 7,
            to: MAIN_ACCOUNT.to_string()
        });
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
        );
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00) + dec!(1500.00) - dec!(123.45));
    }

    #[test]
    fn test_compute_next_day_balances_with_salary_not_on_salary_day() {
        let mut config = create_test_accounts(10);
        config.transactions.push(Transaction::Salary {
            amount: dec!(1000.00),
            day: 15,
            to: MAIN_ACCOUNT.to_string()
        });
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
        );
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00) + dec!(1000.00));
    }

    
    #[test]
    fn test_compute_next_day_balances_with_salary_none() {
        let next = make_accounts_for_day(20, 20);
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00) - dec!(123.45));
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
        let account = config.accounts.get(MAIN_ACCOUNT).unwrap();
        assert_eq!(*account, dec!(10000.00));
    }

    #[test]
    fn test_run_balances_consistency() {
        let config = create_test_accounts(1);
        println!("Config: {:#?}", config);
        let balances = config.accounts.clone();
        let days = 30; // Run for 30 days
        let history = super::run(&config, balances, days);
        let final_balances = history.last().expect("History should not be empty").1.clone();
        // The sum of all balances should be zero (by design)
        let total: Decimal = final_balances.values().copied().sum();
        assert_eq!(total, Decimal::ZERO);
        assert_eq!(final_balances[MAIN_ACCOUNT], dec!(10000.00) + dec!(2000.00));
        assert_eq!(history.len(), days as usize, "History should have one entry per day");
    }

    #[test]
    fn test_run_final_balances_after_salary_and_mortgage() {
        let config = create_test_accounts(1);
        let balances = config.accounts.clone();
        let days = 6; // On day 6, salary is paid
        let history = super::run(&config, balances, days);
        let final_balances = &history.last().unwrap().1;
        // Salary should be added on day 6
        assert_eq!(final_balances[MAIN_ACCOUNT], dec!(10000.00) + dec!(2000.00));
    }

    #[test]
    fn test_run_mortgage_deduction_applied() {
        let config = create_test_accounts(3);
        let balances = config.accounts.clone();
        let days = 3; // On day 3, mortgage is deducted
        let history = super::run(&config, balances, days);
        let final_balances = &history.last().unwrap().1;
        // Mortgage should be deducted on day 3
        assert_eq!(final_balances[MAIN_ACCOUNT], dec!(10000.00) - dec!(123.45));
    }

    #[test]
    fn test_run_balances_sum_to_zero_each_day() {
        let config = create_test_accounts(1);
        let balances = config.accounts.clone();
        let days = 15;
        let history = super::run(&config, balances, days);
        for (date, balances) in history {
            let total: Decimal = balances.values().copied().sum();
            assert_eq!(total, Decimal::ZERO, "Balances do not sum to zero on {date}");
        }
    }

    #[test]
    fn test_run_salary_paid_on_correct_day() {
        let config = create_test_accounts(15);
        let balances = config.accounts.clone();
        let days = 10;
        let history = super::run(&config, balances, days);
        // Salary is paid on day 6, so check balance before and after
        // get the salary day from config
        assert!(config.transactions.len() > 1, "Config should have at least two transactions");
        assert!(matches!(config.transactions[1], Transaction::Salary { .. }), "Second transaction should be a Salary transaction");
        let salary_day = if let Transaction::Salary { day, .. } = &config.transactions[1] {
            *day
        } else {
            panic!("Expected second transaction to be a Salary transaction");
        };
        assert_eq!(salary_day, 6, "Salary day should be 6");
        let before = &history[salary_day as usize - 3]; // day 5
        let after = &history[salary_day as usize - 2];        
        assert_eq!(before.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap());
        assert_eq!(after.0, chrono::NaiveDate::from_ymd_opt(2025, 1, 6).unwrap());
        // Check salary was paid correctly
        let before_salary  = &before.1[MAIN_ACCOUNT]; // day 5
        let after_salary = &after.1[MAIN_ACCOUNT];  // day 6
        // print history
        println!("History: {:#?}", history);
        assert_eq!(
            *after_salary,
            *before_salary + dec!(2000.00),
            "Salary not paid correctly on salary day"
        );
    }    


    #[test]
    fn test_run_multiple_transactions_same_day() {
        let mut config = create_test_accounts(3);
        config.transactions.push(Transaction::Salary {
            amount: dec!(500.00),
            day: 3,
            to: MAIN_ACCOUNT.to_string(),
        });
        let balances = config.accounts.clone();
        let days = 3;
        let history = super::run(&config, balances, days);
        let final_balances = &history.last().unwrap().1;
        // On day 3, both mortgage and salary should be applied
        assert_eq!(final_balances[MAIN_ACCOUNT], dec!(10000.00) - dec!(123.45) + dec!(500.00));
    }

    #[test]
    fn test_run_salary_to_different_account() {
        let mut config = create_test_accounts(6);
        // Add a new account and pay salary to it
        let alt_account = "alt_account";
        let mut accounts = config.accounts.clone();
        accounts.insert(alt_account.to_string(), dec!(0.00));
        config.accounts = accounts;
        config.transactions[1] = Transaction::Salary {
            amount: dec!(2000.00),
            day: 6,
            to: alt_account.to_string(),
        };
        let balances = config.accounts.clone();
        let days = 6;
        let history = super::run(&config, balances, days);
        let final_balances = &history.last().unwrap().1;
        assert_eq!(final_balances[alt_account], dec!(2000.00));
        assert_eq!(final_balances[MAIN_ACCOUNT], dec!(10000.00) -dec!(123.45)); // mortgage deducted
    }
}



