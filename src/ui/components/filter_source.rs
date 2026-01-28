/// Trait for filtering a list of items by a value.
///
/// Multiple filter sources can work on the same item type `T`.
/// For example, you could have separate filter sources for filtering
/// issues by assignee vs by epic.
pub trait FilterSource<T>: Clone + Default + PartialEq + 'static {
  /// Human-readable label for this filter source
  fn label(&self) -> &'static str;

  /// Get unique values from the list for populating filter tabs.
  /// Returns `None` values for items with missing field values.
  fn unique_values(&self, items: &[T]) -> Vec<Option<String>>;

  /// Filter items by a specific value.
  /// - `None` means "All" (no filtering)
  /// - `Some(None)` means filter to items with missing field value
  /// - `Some(Some(v))` means filter to items matching value v
  fn filter<'a>(&self, items: &'a [T], value: Option<&Option<String>>) -> Vec<&'a T>;

  /// Check if this filter source is active (not "None")
  fn is_active(&self) -> bool;

  /// Get all available filter source variants
  fn all_variants() -> &'static [Self];
}
