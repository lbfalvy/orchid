fn string_from_charset_rec(val: usize, digits: &str) -> String {
    let radix = digits.len();
    let mut prefix = if val > radix {
        string_from_charset_rec(val / radix, digits)
    } else {String::new()};
    prefix.push(digits.chars().nth(val - 1).unwrap_or_else(|| {
        panic!("Overindexed digit set \"{}\" with {}", digits, val - 1)
    }));
    prefix
}

pub fn string_from_charset(val: usize, digits: &str) -> String {
    string_from_charset_rec(val + 1, digits)
}