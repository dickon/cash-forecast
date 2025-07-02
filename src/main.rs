use chrono::Datelike;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct Config {
    mortgage: Mortgage,
    initial_balance: Decimal,
    #[serde(default = "default_currency_symbol")]
    currency_symbol: String,
}

#[derive(Debug, Deserialize)]
struct Mortgage {
    deduction_amount: Decimal,
    deduction_day: u32,
}

fn default_currency_symbol() -> String {
    "£".to_string()
}

fn main() {
    // Load config from YAML
    let yaml = fs::read_to_string("config.yaml").expect("Failed to read config.yaml");
    let config: Config = serde_yaml::from_str(&yaml).expect("Failed to parse YAML");

    let today = chrono::Local::now().date_naive();

    let balance = (today, config.initial_balance);

    let mut baln = balance;

    for _ in 0..60 {
        baln = (baln.0 + chrono::Duration::days(1), baln.1);
        if baln.0.day() == config.mortgage.deduction_day {
            baln.1 -= config.mortgage.deduction_amount;
        }
        print_balance(baln, &config.currency_symbol);
    }
}

fn print_balance(balance: (chrono::NaiveDate, Decimal), currency_symbol: &str) {
    let date = balance.0;
    // Format to 2 decimal places and prefix with £
    println!("{date} {symbol}{v:.2}", date = date, symbol = currency_symbol, v = balance.1);
}

