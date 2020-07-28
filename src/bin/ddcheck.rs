use std::env;
use weblite::descriptions;

pub fn main() {
    env::args()
        .collect::<Vec<String>>()
        .iter()
        .skip(1)
        .inspect(|path| print!("Checking path: {} ", path))
        .map(|path| descriptions::read_def(path))
        .for_each(|res| match res {
            Ok(_) => println!("OK"),
            Err(err) => println!("ERROR!\n  {}", err),
        });
}
