/// Case-insensitive AND-match of whitespace-separated keywords in `query`
/// against `haystack`. Empty/whitespace query matches everything.
pub fn keyword_match(haystack: &str, query: &str) -> bool {
  let query = query.trim().to_lowercase();
  if query.is_empty() {
    return true;
  }
  let haystack = haystack.to_lowercase();
  query.split_whitespace().all(|kw| haystack.contains(kw))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn empty_query_matches() {
    assert!(keyword_match("anything", ""));
    assert!(keyword_match("anything", "   "));
  }

  #[test]
  fn single_keyword_substring() {
    assert!(keyword_match("PROJ-123 fix login bug", "login"));
    assert!(!keyword_match("PROJ-123 fix bug", "login"));
  }

  #[test]
  fn multiple_keywords_out_of_order() {
    assert!(keyword_match("PROJ-123 fix login bug", "bug login"));
    assert!(keyword_match("PROJ-123 fix login bug", "login bug"));
    assert!(keyword_match("PROJ-123 fix login bug", "proj fix"));
  }

  #[test]
  fn case_insensitive() {
    assert!(keyword_match("Fix Login Bug", "LOGIN"));
    assert!(keyword_match("Fix Login Bug", "fix BUG"));
  }

  #[test]
  fn missing_keyword_fails() {
    assert!(!keyword_match("fix login bug", "login auth"));
  }
}
