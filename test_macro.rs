macro_rules! eprintln {
    ($($arg:tt)*) => { () };
}

fn main() {
    eprintln!("This should not print: {}", 42);
    let x = move || eprintln!("foo");
}
