macro_rules! eprintln {
    ($($arg:tt)*) => { () };
}
fn diagnose() {
    eprintln!("this should be silenced");
}
fn main() {
    diagnose();
}
