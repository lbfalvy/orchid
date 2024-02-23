//! Generate valid names from numbers and a character set. For small numbers,
//! the results should be substantially more memorable than the plain numbers.

fn string_from_charset_rec(val: u64, digits: &str) -> String {
  let radix = digits.len() as u64;
  let mut prefix =
    if val > radix { string_from_charset_rec(val / radix, digits) } else { String::new() };
  let digit = digits
    .chars()
    .nth(val as usize - 1)
    .unwrap_or_else(|| panic!("Overindexed digit set \"{}\" with {}", digits, val - 1));
  prefix.push(digit);
  prefix
}

/// Generate alphabetized names from numbers using a set of permitted
/// characters. Especially practical in combination with De Bruijn indices
pub fn string_from_charset(val: u64, digits: &str) -> String {
  string_from_charset_rec(val + 1, digits)
}
