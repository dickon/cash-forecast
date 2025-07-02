fn main() {
    // work out todays date
    let today = chrono::Local::now();
    println!("Today's date is: {}", today.format("%Y-%m-%d"));

    let x = 2;
    println!("Hello, world! {x}");
}
