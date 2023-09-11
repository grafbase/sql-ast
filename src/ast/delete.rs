use crate::ast::{ConditionTree, Query, Table};

#[derive(Debug, PartialEq, Clone)]
/// A builder for a `DELETE` statement.
pub struct Delete<'a> {
    pub(crate) table: Table<'a>,
    pub(crate) conditions: Option<ConditionTree<'a>>,
}

impl<'a> From<Delete<'a>> for Query<'a> {
    fn from(delete: Delete<'a>) -> Self {
        Query::Delete(Box::new(delete))
    }
}

impl<'a> Delete<'a> {
    /// Creates a new `DELETE` statement for the given table.
    ///
    /// ```rust
    /// # use grafbase_sql_ast::{ast::*, renderer::{Renderer, self}};
    /// # fn main() {
    /// let query = Delete::from_table("users");
    /// let (sql, _) = renderer::Postgres::build(query);
    ///
    /// assert_eq!(r#"DELETE FROM "users""#, sql);
    /// # }
    /// ```
    pub fn from_table<T>(table: T) -> Self
    where
        T: Into<Table<'a>>,
    {
        Self {
            table: table.into(),
            conditions: None,
        }
    }

    /// Adds `WHERE` conditions to the query. See
    /// [Comparable](trait.Comparable.html#required-methods) for more examples.
    ///
    /// ```rust
    /// # use grafbase_sql_ast::{ast::*, renderer::{Renderer, self}};
    /// # fn main() {
    /// let query = Delete::from_table("users").so_that("bar".equals(false));
    /// let (sql, params) = renderer::Postgres::build(query);
    ///
    /// assert_eq!(r#"DELETE FROM "users" WHERE "bar" = $1"#, sql);
    /// assert_eq!(vec![Value::from(false)], params);
    /// # }
    /// ```
    pub fn so_that<T>(mut self, conditions: T) -> Self
    where
        T: Into<ConditionTree<'a>>,
    {
        self.conditions = Some(conditions.into());
        self
    }
}
