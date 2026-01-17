/// Available commands and autocomplete logic

#[derive(Debug, Clone)]
pub struct Command {
  pub name: &'static str,
  pub aliases: &'static [&'static str],
  pub description: &'static str,
}

/// All available commands
pub const COMMANDS: &[Command] = &[
  Command {
    name: "issues",
    aliases: &["i", "issue"],
    description: "Browse project issues",
  },
  Command {
    name: "boards",
    aliases: &["b", "board"],
    description: "View agile boards",
  },
  Command {
    name: "epics",
    aliases: &["e", "epic"],
    description: "Browse epics",
  },
  Command {
    name: "searches",
    aliases: &["s", "search", "filters"],
    description: "Saved searches/filters",
  },
  Command {
    name: "quit",
    aliases: &["q", "exit"],
    description: "Exit j9s",
  },
];

/// Get autocomplete suggestions for a given input
pub fn get_suggestions(input: &str) -> Vec<&'static Command> {
  let input_lower = input.to_lowercase();

  if input_lower.is_empty() {
    return COMMANDS.iter().collect();
  }

  let mut matches: Vec<(&Command, u32)> = Vec::new();

  for cmd in COMMANDS {
    // Exact match on name
    if cmd.name == input_lower {
      matches.push((cmd, 0)); // Highest priority
      continue;
    }

    // Exact match on alias
    if cmd.aliases.contains(&input_lower.as_str()) {
      matches.push((cmd, 1));
      continue;
    }

    // Prefix match on name
    if cmd.name.starts_with(&input_lower) {
      matches.push((cmd, 2));
      continue;
    }

    // Prefix match on alias
    if cmd.aliases.iter().any(|a| a.starts_with(&input_lower)) {
      matches.push((cmd, 3));
      continue;
    }

    // Fuzzy match (contains)
    if cmd.name.contains(&input_lower) {
      matches.push((cmd, 4));
      continue;
    }

    // Fuzzy match on alias
    if cmd.aliases.iter().any(|a| a.contains(&input_lower)) {
      matches.push((cmd, 5));
    }
  }

  // Sort by priority
  matches.sort_by_key(|(_, priority)| *priority);

  matches.into_iter().map(|(cmd, _)| cmd).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_empty_input_returns_all() {
    let suggestions = get_suggestions("");
    assert_eq!(suggestions.len(), COMMANDS.len());
  }

  #[test]
  fn test_exact_match() {
    let suggestions = get_suggestions("issues");
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "issues");
  }

  #[test]
  fn test_alias_match() {
    let suggestions = get_suggestions("i");
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "issues");
  }

  #[test]
  fn test_prefix_match() {
    let suggestions = get_suggestions("iss");
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "issues");
  }

  #[test]
  fn test_fuzzy_match() {
    let suggestions = get_suggestions("sue");
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0].name, "issues");
  }
}
