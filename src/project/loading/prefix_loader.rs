use super::Loader;

pub fn prefix_loader<'a>(
  prefix: &'a [&'a str], mut loader: impl Loader + 'a
) -> impl Loader + 'a {
  move |path: &[&str]| {
    let full_path = prefix.iter().chain(path.iter()).map(|s| s.to_string()).clone();
    loader.load(path)
  }
}