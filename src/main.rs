use chrono::Datelike;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;
use std::fs;

const MAIN_ACCOUNT: &str = "main";
const SALARY_INCOME: &str = "salary_income";
const MORTGAGE_INCOME: &str = "mortgage_income";
const MORTGAGE_ACCOUNT: &str = "mortgage";
const OPENING_BALANCES: &str = "opening_balances";
const CHARITY_EXPENDITURE: &str = "charity_expenditure";

#[derive(Debug, Deserialize, PartialEq)]
struct Config {
    transactions: Vec<Generator>,
    accounts: std::collections::HashMap<String, Decimal>,
    #[serde(default = "default_currency_symbol")]
    currency_symbol: String,
    #[serde(default = "default_start_date")]
    start_date: chrono::NaiveDate,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Generator {
    #[serde(rename = "mortgage")]
    Mortgage {
        deduction_amount: Decimal,
        deduction_day: u32,
        #[serde(default = "default_main")]
        from: String,
        #[serde(default = "default_mortgage")]
        to: String,
    },
    #[serde(rename = "interest")]
    Interest {
        rate: Decimal,
        day: u32,
        #[serde(default = "default_mortgage")]
        account: String,
        #[serde(default = "default_mortgage_income")]
        income_account: String,
    },
    #[serde(rename = "salary")]
    Salary {
        amount: Decimal,
        day: u32,
        #[serde(default = "default_main")]
        to: String,
    },
    #[serde(rename = "transfer")]
    Transfer {
        amount: Decimal,
        day: u32,
        #[serde(default = "default_main")]
        from: String,
        #[serde(default = "default_main")]
        to: String,
    },
    #[serde(rename = "tithe")]
    Tithe {
        percentage: Decimal,
        day: u32,
        #[serde(default = "default_main")]
        from: String,
        #[serde(default = "default_charity")]
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

fn default_mortgage_income() -> String {
    MORTGAGE_INCOME.to_string()
}

fn default_charity() -> String {
    CHARITY_EXPENDITURE.to_string()
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

    let history = run(&config, balances, 6000);
    
    // Print the history of balances
    for (date, balances) in &history {
        if date.day() == 1 {
            println!("\nBalances on {date}:");
            for (name, balance) in balances {
                print_balance_named(name, *date, *balance, &config.currency_symbol); 
            }
        }
    }
    
    // Create plots for mortgage balance over time
    create_mortgage_plots(&history, &config.currency_symbol);
}

fn run(
    config: &Config,
    balances: std::collections::HashMap<String, Decimal>,
    days_to_run: i32
) -> Vec<(chrono::NaiveDate, std::collections::HashMap<String, Decimal>)> {
    let mut balances = balances;
    let mut date: chrono::NaiveDate = config.start_date;
    let mut history = Vec::new();
    let mut total_salary_since_last_tithe = Decimal::ZERO;

    for _ in 0..days_to_run {
        date = date + chrono::Duration::days(1);
        let (new_balances, new_total_salary) = compute_next_day_balances_with_tithe(config, &balances, date, total_salary_since_last_tithe);
        balances = new_balances;
        total_salary_since_last_tithe = new_total_salary;
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
    if !new_balances.contains_key(CHARITY_EXPENDITURE) {
        new_balances.insert(CHARITY_EXPENDITURE.to_string(), Decimal::ZERO);
    }
    new_balances
}

fn compute_next_day_balances_with_tithe(
    config: &Config,
    balances: &std::collections::HashMap<String, Decimal>,
    date: chrono::NaiveDate,
    total_salary_since_last_tithe: Decimal,
) -> (std::collections::HashMap<String, Decimal>, Decimal) {
    let mut new_balances = balances.clone();
    let mut salary_accumulator = total_salary_since_last_tithe;

    // For each transaction, apply its effect to the relevant accounts
    for transaction in &config.transactions {
        match transaction {
            Generator::Mortgage { deduction_amount, deduction_day, from, to } => {
                if date.day() == *deduction_day {
                    let from_balance = *new_balances.get(from).expect("From account not found in balances");
                    let to_balance = *new_balances.get(to).expect("to account not found in balances");
                    assert!(to_balance <= Decimal::ZERO, "Mortgage account must be negative; is {to_balance}");
                    let actual_deduction = (*deduction_amount).min(-to_balance).min(from_balance).max(Decimal::ZERO);
                    assert!(actual_deduction <= *deduction_amount);
                    assert!(actual_deduction >= Decimal::ZERO, "Mortgage deduction amount must be non-negative; is {actual_deduction}");
                    *new_balances.get_mut(from).expect("From account not found in balances") -= actual_deduction;
                    *new_balances.get_mut(to).expect("To account not found in balances") += actual_deduction;
                }
            }
            Generator::Interest { rate, day, account, income_account } => {
                if date.day() == *day && *rate != Decimal::ZERO {
                    let current_balance = *new_balances.get(account).unwrap();
                    let monthly_interest_exact = current_balance * (*rate / dec!(12) / dec!(100));
                    // round monthly interest to 2 decimal places
                    let monthly_interest = monthly_interest_exact.round_dp(2);
                    *new_balances.get_mut(account).expect("Account not found for interest") += monthly_interest;
                    *new_balances.get_mut(income_account).expect("Income account not found for interest") -= monthly_interest;
                }
            }
            Generator::Salary { amount, day, to } => {
                if date.day() == *day {
                    *new_balances.get_mut(to).expect("Salary 'to' account not found") += *amount;
                    *new_balances.get_mut(SALARY_INCOME).expect("salary_income not found for salary") -= *amount;
                    // Accumulate salary for tithe calculation
                    salary_accumulator += *amount;
                }
            }
            Generator::Transfer { amount, day, from, to } => {
                if date.day() == *day {
                    *new_balances.get_mut(from).expect("Transfer 'from' account not found") -= *amount;
                    *new_balances.get_mut(to).expect("Transfer 'to' account not found") += *amount;
                }
            }
            Generator::Tithe { percentage, day, from, to } => {
                if date.day() == *day {
                    // Calculate tithe amount as percentage of accumulated salary
                    let tithe_amount = (salary_accumulator * *percentage / dec!(100)).round_dp(2);
                    if tithe_amount > Decimal::ZERO {
                        *new_balances.get_mut(from).expect("Tithe 'from' account not found") -= tithe_amount;
                        *new_balances.get_mut(to).expect("Tithe 'to' account not found") += tithe_amount;
                        // Reset salary accumulator after tithe is paid
                        salary_accumulator = Decimal::ZERO;
                    }
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
    (new_balances, salary_accumulator)
}

fn compute_next_day_balances(
    config: &Config,
    balances: &std::collections::HashMap<String, Decimal>,
    date: chrono::NaiveDate,
) -> std::collections::HashMap<String, Decimal> {
    let (new_balances, _) = compute_next_day_balances_with_tithe(config, balances, date, Decimal::ZERO);
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

fn create_mortgage_plots(
    history: &[(chrono::NaiveDate, std::collections::HashMap<String, Decimal>)],
    currency_symbol: &str,
) {
    // Extract dates and mortgage balances
    let mut csv_lines = vec!["Date,Balance".to_string()];
    
    for (date, balances) in history {
        if let Some(mortgage_balance) = balances.get(MORTGAGE_ACCOUNT) {
            csv_lines.push(format!("{},{}", date.format("%Y-%m-%d"), mortgage_balance));
        }
    }
    
    // Create CSV file
    if let Err(e) = std::fs::write("/tmp/mortgage_balance.csv", csv_lines.join("\n")) {
        eprintln!("Error creating CSV file: {}", e);
    } else {
        println!("Mortgage balance CSV data saved to '/tmp/mortgage_balance.csv'");
    }
    
    // Create HTML plot with Chart.js
    create_html_chart(&csv_lines, currency_symbol);
}

fn create_html_chart(csv_lines: &[String], currency_symbol: &str) {
    // Skip header and extract data for JavaScript
    let data_lines: Vec<&str> = csv_lines.iter().skip(1).map(|s| s.as_str()).collect();
    
    let mut dates = Vec::new();
    let mut balances = Vec::new();
    
    for line in data_lines {
        if let Some((date, balance)) = line.split_once(',') {
            dates.push(format!("'{}'", date));
            balances.push(balance.to_string());
        }
    }
    
    let html_content = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Mortgage Balance Over Time</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        .chart-container {{ width: 90%; height: 400px; margin: 0 auto; }}
        h1 {{ text-align: center; }}
    </style>
</head>
<body>
    <h1>Mortgage Balance Over Time</h1>
    <div class="chart-container">
        <canvas id="mortgageChart"></canvas>
    </div>
    
    <script>
        const ctx = document.getElementById('mortgageChart').getContext('2d');
        const chart = new Chart(ctx, {{
            type: 'line',
            data: {{
                labels: [{}],
                datasets: [{{
                    label: 'Mortgage Balance ({})',
                    data: [{}],
                    borderColor: 'rgb(75, 192, 192)',
                    backgroundColor: 'rgba(75, 192, 192, 0.2)',
                    tension: 0.1
                }}]
            }},
            options: {{
                responsive: true,
                maintainAspectRatio: false,
                scales: {{
                    y: {{
                        beginAtZero: false,
                        title: {{
                            display: true,
                            text: 'Balance ({})'
                        }}
                    }},
                    x: {{
                        title: {{
                            display: true,
                            text: 'Date'
                        }},
                        ticks: {{
                            maxTicksLimit: 10
                        }}
                    }}
                }}
            }}
        }});
    </script>
</body>
</html>"#,
        dates.join(", "),
        currency_symbol,
        balances.join(", "),
        currency_symbol
    );
    
    if let Err(e) = std::fs::write("/tmp/mortgage_balance.html", html_content) {
        eprintln!("Error creating HTML file: {}", e);
    } else {
        println!("Mortgage balance HTML chart saved to '/tmp/mortgage_balance.html'");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;

    fn create_test_accounts_with_main_balance(mortgage_deduction_day: u32, main_balance: Option<Decimal>) -> Config {
        let main_balance = main_balance.unwrap_or(dec!(10000.00));
        let accounts = HashMap::from([
            (MAIN_ACCOUNT.to_string(), main_balance),
            (MORTGAGE_ACCOUNT.to_string(), dec!(-500000.00)),
        ]);
        let accounts_with_defaults = super::add_default_accounts(&accounts);
        let accounts_with_opening = add_opening_balances(&accounts_with_defaults);
        Config {
            transactions: vec![
                Generator::Mortgage {
                    deduction_amount: dec!(123.45),
                    deduction_day: mortgage_deduction_day,
                    from: MAIN_ACCOUNT.to_string(),
                    to: MORTGAGE_ACCOUNT.to_string(),
                },
                Generator::Interest {
                    rate: dec!(5.0), // 5% annual interest rate
                    day: mortgage_deduction_day,
                    account: MORTGAGE_ACCOUNT.to_string(),
                    income_account: MORTGAGE_INCOME.to_string(),
                },
                Generator::Salary {
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

    // For backward compatibility, keep the original function
    fn create_test_accounts(mortgage_deduction_day: u32) -> Config {
        create_test_accounts_with_main_balance(mortgage_deduction_day, None)
    }

    #[test]
    fn test_config_parsing() {
        let yaml = r#"
transactions:
  - type: mortgage
    deduction_amount: 123.45
    deduction_day: 1
  - type: interest
    rate: 5.0
    day: 1
    account: mortgage
    income_account: mortgage_income
  - type: salary
    amount: 2000.00
    day: 6
accounts:
  main: 10000.00
  mortgage: -500000.00
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
        config.transactions.push(Generator::Salary {
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
        config.transactions.push(Generator::Salary {
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
        assert!(config.transactions.len() > 2, "Config should have at least three transactions");
        assert!(matches!(config.transactions[2], Generator::Salary { .. }), "Third transaction should be a Salary transaction");
        let salary_day = if let Generator::Salary { day, .. } = &config.transactions[2] {
            *day
        } else {
            panic!("Expected third transaction to be a Salary transaction");
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
        config.transactions.push(Generator::Salary {
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
        config.transactions[2] = Generator::Salary {  // Fix: index 2 is the Salary transaction
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

    #[test]
    fn test_transfer_between_accounts() {
        let mut config = create_test_accounts(10); // No mortgage/salary on test day
        let savings_account = "savings";
        
        // Add savings account
        config.accounts.insert(savings_account.to_string(), dec!(0.00));
        
        // Add transfer transaction
        config.transactions.push(Generator::Transfer {
            amount: dec!(500.00),
            day: 5,
            from: MAIN_ACCOUNT.to_string(),
            to: savings_account.to_string(),
        });
        
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(),
        );
        
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00) - dec!(500.00));
        assert_eq!(next[savings_account], dec!(500.00));
    }

    #[test]
    fn test_transfer_not_on_transfer_day() {
        let mut config = create_test_accounts(10);
        let savings_account = "savings";
        
        config.accounts.insert(savings_account.to_string(), dec!(0.00));
        config.transactions.push(Generator::Transfer {
            amount: dec!(500.00),
            day: 7,
            from: MAIN_ACCOUNT.to_string(),
            to: savings_account.to_string(),
        });
        
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(), // Not transfer day
        );
        
        // No transfer should occur
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00));
        assert_eq!(next[savings_account], dec!(0.00));
    }

    #[test]
    fn test_multiple_transfers_same_day() {
        let mut config = create_test_accounts(10);
        let savings_account = "savings";
        let investment_account = "investments";
        
        config.accounts.insert(savings_account.to_string(), dec!(0.00));
        config.accounts.insert(investment_account.to_string(), dec!(0.00));
        
        config.transactions.push(Generator::Transfer {
            amount: dec!(300.00),
            day: 5,
            from: MAIN_ACCOUNT.to_string(),
            to: savings_account.to_string(),
        });
        
        config.transactions.push(Generator::Transfer {
            amount: dec!(200.00),
            day: 5,
            from: MAIN_ACCOUNT.to_string(),
            to: investment_account.to_string(),
        });
        
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(),
        );
        
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00) - dec!(300.00) - dec!(200.00));
        assert_eq!(next[savings_account], dec!(300.00));
        assert_eq!(next[investment_account], dec!(200.00));
    }

    #[test]
    fn test_transfer_with_salary_and_mortgage_same_day() {
        let mut config = create_test_accounts(7); // Mortgage and salary on day 7
        let savings_account = "savings";
        
        config.accounts.insert(savings_account.to_string(), dec!(0.00));
        
        // Change existing transactions to day 7
        config.transactions[0] = Generator::Mortgage {
            deduction_amount: dec!(123.45),
            deduction_day: 7,
            from: MAIN_ACCOUNT.to_string(),
            to: MORTGAGE_ACCOUNT.to_string(),
        };
        config.transactions[1] = Generator::Salary {
            amount: dec!(2000.00),
            day: 7,
            to: MAIN_ACCOUNT.to_string(),
        };
        
        // Add transfer on same day
        config.transactions.push(Generator::Transfer {
            amount: dec!(500.00),
            day: 7,
            from: MAIN_ACCOUNT.to_string(),
            to: savings_account.to_string(),
        });
        
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
        );
        
        // Main account: start + salary - mortgage - transfer
        assert_eq!(next[MAIN_ACCOUNT], dec!(10000.00) + dec!(2000.00) - dec!(123.45) - dec!(500.00));
        assert_eq!(next[savings_account], dec!(500.00));
    }

    #[test]
    fn test_config_parsing_with_transfer() {
        let yaml = r#"
transactions:
  - type: transfer
    amount: 250.00
    day: 10
    from: main
    to: savings
accounts:
  main: 5000.00
  savings: 0.00
currency_symbol: "£"
start_date: "2025-01-01"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.transactions.len(), 1);
        
        if let Generator::Transfer { amount, day, from, to } = &config.transactions[0] {
            assert_eq!(*amount, dec!(250.00));
            assert_eq!(*day, 10);
            assert_eq!(from, "main");
            assert_eq!(to, "savings");
        } else {
            panic!("Expected Transfer transaction");
        }
    }

    #[test]
    fn test_interest_calculation() {
        let mut config = create_test_accounts(5); // Mortgage on day 5, salary on day 6
        // Clear existing interest transaction and add a new one for day 10
        config.transactions = vec![
            Generator::Mortgage {
                deduction_amount: dec!(123.45),
                deduction_day: 5,
                from: MAIN_ACCOUNT.to_string(),
                to: MORTGAGE_ACCOUNT.to_string(),
            },
            Generator::Interest {
                rate: dec!(6.0), // 6% annual rate
                day: 10,
                account: MORTGAGE_ACCOUNT.to_string(),
                income_account: MORTGAGE_INCOME.to_string(),
            },
        ];
        
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
        );
        
        // Calculate expected interest: 500000 * (6% / 12 / 100) = 500000 * 0.005 = 2500
        let expected_interest = dec!(-500000.00) * (dec!(6.0) / dec!(12) / dec!(100));
        assert_eq!(expected_interest, dec!(-2500.00));
        
        // Mortgage balance should increase by interest
        assert_eq!(next[MORTGAGE_ACCOUNT], dec!(-500000.00) + expected_interest);
        
    }

    #[test]
    fn test_mortgage_payment_limited_by_available_balance() {
        let mut config = create_test_accounts_with_main_balance(5, Some(dec!(100.0)));
        // Set up a scenario where the mortgage payment exceeds the available balance
        config.transactions[0] = Generator::Mortgage {
            deduction_amount: dec!(500.00), // Try to deduct £500
            deduction_day: 5,
            from: MAIN_ACCOUNT.to_string(),
            to: MORTGAGE_ACCOUNT.to_string(),
        };
        
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(),
        );
        
        // Should only deduct the available £100, leaving balance at zero
        assert_eq!(next[MAIN_ACCOUNT], dec!(0.00));
        // Mortgage account should only receive the actual deduction amount
        let original_mortgage = config.accounts.get(MORTGAGE_ACCOUNT).unwrap();
        let interest = ((dec!(-500000.00)+dec!(100)) * (dec!(5.0) / dec!(12) / dec!(100))).round_dp(2);
        assert_eq!(next[MORTGAGE_ACCOUNT], *original_mortgage + dec!(100.00) + interest);
    }

    #[test]
    fn test_mortgage_payment_with_negative_balance() {
        // Set up a scenario where the account already has a negative balance
        let mut config = create_test_accounts_with_main_balance(5, Some(dec!(-50.00)));
        config.transactions[0] = Generator::Mortgage {
            deduction_amount: dec!(200.00),
            deduction_day: 5,
            from: MAIN_ACCOUNT.to_string(),
            to: MORTGAGE_ACCOUNT.to_string(),
        };
        
        let next = compute_next_day_balances(
            &config,
            &config.accounts,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 5).unwrap(),
        );
        
        // Should not deduct anything when balance is already negative
        assert_eq!(next[MAIN_ACCOUNT], dec!(-50.00)); // No change
        // Mortgage account should not receive any payment
        let original_mortgage = config.accounts.get(MORTGAGE_ACCOUNT).unwrap();
        // work out interest based on original mortgage balance
        let interest = (dec!(-500000.00) * (dec!(5.0) / dec!(12) / dec!(100))).round_dp(2);
        assert_eq!(next[MORTGAGE_ACCOUNT], *original_mortgage + interest);

    }

    #[test]
    fn test_tithe_calculation_basic() {
        let mut config = create_test_accounts(15); // No mortgage/salary on test day
        
        // Add tithe transaction
        config.transactions.push(Generator::Tithe {
            percentage: dec!(10.0), // 10% tithe
            day: 10,
            from: MAIN_ACCOUNT.to_string(),
            to: CHARITY_EXPENDITURE.to_string(),
        });
        
        // Simulate running for 10 days with salary accumulation
        let balances = config.accounts.clone();
        let history = super::run(&config, balances, 10);
        
        // Get balances on day 10 (when tithe is paid)
        let day_10_balances = &history[9].1; // 0-indexed, so day 10 is index 9
        
        // Salary should be paid on day 6, so by day 10 we should have one salary payment
        // Tithe should be 10% of £2000 = £200
        let expected_tithe = dec!(2000.00) * dec!(10.0) / dec!(100);
        assert_eq!(expected_tithe, dec!(200.00));
        
        // Main account should have: initial + salary - tithe
        assert_eq!(day_10_balances[MAIN_ACCOUNT], dec!(10000.00) + dec!(2000.00) - expected_tithe);
        
        // Charity account should have the tithe amount
        assert_eq!(day_10_balances[CHARITY_EXPENDITURE], expected_tithe);
    }

    #[test]
    fn test_tithe_multiple_salaries() {
        // Create a simple config without mortgage/interest transactions to avoid conflicts
        let accounts = HashMap::from([
            (MAIN_ACCOUNT.to_string(), dec!(10000.00)),
        ]);
        let accounts_with_defaults = super::add_default_accounts(&accounts);
        let accounts_with_opening = add_opening_balances(&accounts_with_defaults);
        
        let config = Config {
            transactions: vec![
                Generator::Salary {
                    amount: dec!(2000.00),
                    day: 6,
                    to: MAIN_ACCOUNT.to_string(),
                },
                Generator::Salary {
                    amount: dec!(1500.00),
                    day: 15,
                    to: MAIN_ACCOUNT.to_string(),
                },
                Generator::Tithe {
                    percentage: dec!(10.0), // 10% tithe
                    day: 20,
                    from: MAIN_ACCOUNT.to_string(),
                    to: CHARITY_EXPENDITURE.to_string(),
                },
            ],
            accounts: accounts_with_opening,
            currency_symbol: "£".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        };
        
        let balances = config.accounts.clone();
        let history = super::run(&config, balances, 20);
        
        // Get balances on day 20
        let day_20_balances = &history[19].1;
        
        // Total salary: £2000 (day 6) + £1500 (day 15) = £3500
        // Tithe: 10% of £3500 = £350
        let total_salary = dec!(2000.00) + dec!(1500.00);
        let expected_tithe = total_salary * dec!(10.0) / dec!(100);
        assert_eq!(expected_tithe, dec!(350.00));
        
        // Main account should have: initial + total_salary - tithe
        assert_eq!(day_20_balances[MAIN_ACCOUNT], dec!(10000.00) + total_salary - expected_tithe);
        
        // Charity account should have the tithe amount
        assert_eq!(day_20_balances[CHARITY_EXPENDITURE], expected_tithe);
    }

    #[test]
    fn test_tithe_resets_salary_accumulator() {
        let mut config = create_test_accounts(30);
        
        // Add multiple tithe transactions
        config.transactions.push(Generator::Tithe {
            percentage: dec!(10.0),
            day: 10,
            from: MAIN_ACCOUNT.to_string(),
            to: CHARITY_EXPENDITURE.to_string(),
        });
        
        config.transactions.push(Generator::Salary {
            amount: dec!(1000.00),
            day: 15,
            to: MAIN_ACCOUNT.to_string(),
        });
        
        config.transactions.push(Generator::Tithe {
            percentage: dec!(10.0),
            day: 20,
            from: MAIN_ACCOUNT.to_string(),
            to: CHARITY_EXPENDITURE.to_string(),
        });
        
        let balances = config.accounts.clone();
        let history = super::run(&config, balances, 20);
        
        // Check day 10 - should tithe on first salary only (£2000 from day 6)
        let day_10_balances = &history[9].1;
        let first_tithe = dec!(2000.00) * dec!(10.0) / dec!(100);
        assert_eq!(day_10_balances[CHARITY_EXPENDITURE], first_tithe);
        
        // Check day 20 - should tithe only on salary from day 15 (£1000)
        let day_20_balances = &history[19].1;
        let second_tithe = dec!(1000.00) * dec!(10.0) / dec!(100);
        let total_tithe = first_tithe + second_tithe;
        assert_eq!(day_20_balances[CHARITY_EXPENDITURE], total_tithe);
        
        // Main account should reflect both tithes
        assert_eq!(day_20_balances[MAIN_ACCOUNT], 
                   dec!(10000.00) + dec!(2000.00) + dec!(1000.00) - total_tithe);
    }

    #[test]
    fn test_tithe_with_zero_salary() {
        let mut config = create_test_accounts(20); // No salary on tithe day
        
        // Remove the default salary transaction
        config.transactions = vec![
            Generator::Tithe {
                percentage: dec!(10.0),
                day: 10,
                from: MAIN_ACCOUNT.to_string(),
                to: CHARITY_EXPENDITURE.to_string(),
            }
        ];
        
        let balances = config.accounts.clone();
        let history = super::run(&config, balances, 10);
        
        // Get balances on day 10
        let day_10_balances = &history[9].1;
        
        // No salary, so no tithe should be paid
        assert_eq!(day_10_balances[CHARITY_EXPENDITURE], dec!(0.00));
        assert_eq!(day_10_balances[MAIN_ACCOUNT], dec!(10000.00)); // No change
    }

    #[test]
    fn test_config_parsing_with_tithe() {
        let yaml = r#"
transactions:
  - type: tithe
    percentage: 10.0
    day: 15
    from: main
    to: charity_expenditure
accounts:
  main: 5000.00
currency_symbol: "£"
start_date: "2025-01-01"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(config.transactions.len(), 1);
        
        if let Generator::Tithe { percentage, day, from, to } = &config.transactions[0] {
            assert_eq!(*percentage, dec!(10.0));
            assert_eq!(*day, 15);
            assert_eq!(from, "main");
            assert_eq!(to, "charity_expenditure");
        } else {
            panic!("Expected Tithe transaction");
        }
    }

    #[test]
    fn test_tithe_with_different_percentage() {
        let mut config = create_test_accounts(15);
        
        config.transactions.push(Generator::Tithe {
            percentage: dec!(5.0), // 5% tithe
            day: 10,
            from: MAIN_ACCOUNT.to_string(),
            to: CHARITY_EXPENDITURE.to_string(),
        });
        
        let balances = config.accounts.clone();
        let history = super::run(&config, balances, 10);
        
        let day_10_balances = &history[9].1;
        
        // Tithe should be 5% of £2000 = £100
        let expected_tithe = dec!(2000.00) * dec!(5.0) / dec!(100);
        assert_eq!(expected_tithe, dec!(100.00));
        
        assert_eq!(day_10_balances[CHARITY_EXPENDITURE], expected_tithe);
        assert_eq!(day_10_balances[MAIN_ACCOUNT], dec!(10000.00) + dec!(2000.00) - expected_tithe);
    }
}



