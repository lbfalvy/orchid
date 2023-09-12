use chumsky::prelude::*;

use super::decls::SimpleParser;

pub fn operators_parser<T>(
  f: impl Fn(String) -> T,
) -> impl SimpleParser<char, Vec<T>> {
  filter(|c: &char| c != &']' && !c.is_whitespace())
    .repeated()
    .at_least(1)
    .collect()
    .map(f)
    .separated_by(text::whitespace())
    .allow_leading()
    .allow_trailing()
    .at_least(1)
    .delimited_by(just("operators["), just(']'))
}

#[cfg(test)]
mod test {
  use chumsky::Parser;

  use super::operators_parser;

  #[test]
  fn operators_scratchpad() {
    let parsely = operators_parser(|s| s);
    println!("{:?}", parsely.parse("operators[$ |> =>]"))
  }
}
