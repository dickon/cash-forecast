fn main() {
    // work out todays date as a Date, not a DateTime
    // this is so that we can add days to it without worrying about the time of day
    // we can then use the DateTime to print the date in a human readable format
    // and to calculate the balance in a human readable format      

    
    let today = chrono::Local::now().date_naive();

    let balance = (today, 10000*100);

    let bal2 = (balance.0 + chrono::Duration::days(1), balance.1 - 438912);


    print_balance(balance);
    print_balance(bal2);
}


fn print_balance(balance: (chrono::NaiveDate, i32)) {
    let date = balance.0;
    let v = balance.1 as f64 / 100.0;
    println!("{date} {v}");
}

