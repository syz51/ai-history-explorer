/// Filter field types supported in Phase 2
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterField {
    /// Filter by project path (supports ~ and partial matches)
    Project,
    /// Filter by entry type (user or agent)
    Type,
    /// Filter entries after date (YYYY-MM-DD format)
    Since,
}

/// Logical operators for combining filters
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOperator {
    /// Both conditions must match (default between different fields)
    And,
    /// Either condition matches (default within same field)
    Or,
}

/// Single field:value filter
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldFilter {
    pub field: FilterField,
    pub value: String,
}

impl FieldFilter {
    pub fn new(field: FilterField, value: String) -> Self {
        Self { field, value }
    }
}

/// Filter expression combining multiple field filters with operators
///
/// Phase 2 limitation: No parentheses support
/// - Same-field filters are OR'd together: project:foo project:bar → (foo OR bar)
/// - Cross-field filters are AND'd together: project:foo type:user → (foo AND user)
/// - Explicit operators override defaults
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterExpr {
    pub filters: Vec<FieldFilter>,
    pub operators: Vec<FilterOperator>,
}

impl FilterExpr {
    pub fn new() -> Self {
        Self { filters: Vec::new(), operators: Vec::new() }
    }

    pub fn add_filter(&mut self, filter: FieldFilter) {
        self.filters.push(filter);
    }

    pub fn add_operator(&mut self, operator: FilterOperator) {
        self.operators.push(operator);
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

impl Default for FilterExpr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_filter_creation() {
        let filter = FieldFilter::new(FilterField::Project, "foo".to_string());
        assert_eq!(filter.field, FilterField::Project);
        assert_eq!(filter.value, "foo");
    }

    #[test]
    fn test_filter_expr_empty() {
        let expr = FilterExpr::new();
        assert!(expr.is_empty());
        assert_eq!(expr.filters.len(), 0);
        assert_eq!(expr.operators.len(), 0);
    }

    #[test]
    fn test_filter_expr_add() {
        let mut expr = FilterExpr::new();
        expr.add_filter(FieldFilter::new(FilterField::Project, "foo".to_string()));
        assert!(!expr.is_empty());
        assert_eq!(expr.filters.len(), 1);
    }

    #[test]
    fn test_filter_expr_with_operators() {
        let mut expr = FilterExpr::new();
        expr.add_filter(FieldFilter::new(FilterField::Project, "foo".to_string()));
        expr.add_operator(FilterOperator::And);
        expr.add_filter(FieldFilter::new(FilterField::Type, "user".to_string()));
        assert_eq!(expr.filters.len(), 2);
        assert_eq!(expr.operators.len(), 1);
        assert_eq!(expr.operators[0], FilterOperator::And);
    }
}
