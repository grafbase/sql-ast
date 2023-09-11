use std::borrow::Cow;

use crate::ast::{Column, Expression, Query, Row, Table, Update, Values};

/// A builder for an `INSERT` statement.
#[derive(Clone, Debug, PartialEq)]
pub struct Insert<'a> {
    pub(crate) table: Option<Table<'a>>,
    pub(crate) columns: Vec<Column<'a>>,
    pub(crate) values: Expression<'a>,
    pub(crate) on_conflict: Option<OnConflict<'a>>,
    pub(crate) returning: Option<Vec<Column<'a>>>,
    pub(crate) comment: Option<Cow<'a, str>>,
}

/// A builder for an `INSERT` statement for a single row.
#[derive(Clone, Debug, PartialEq)]
pub struct SingleRowInsert<'a> {
    pub(crate) table: Option<Table<'a>>,
    pub(crate) columns: Vec<Column<'a>>,
    pub(crate) values: Row<'a>,
}

/// A builder for an `INSERT` statement for multiple rows.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiRowInsert<'a> {
    pub(crate) table: Option<Table<'a>>,
    pub(crate) columns: Vec<Column<'a>>,
    pub(crate) values: Vec<Row<'a>>,
}

/// `INSERT` conflict resolution strategies.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum OnConflict<'a> {
    /// When a row already exists, do nothing.
    DoNothing,
    /// ON CONFLICT UPDATE is supported for Postgres
    Update(Update<'a>, Vec<Column<'a>>),
}

impl<'a> From<Insert<'a>> for Query<'a> {
    fn from(insert: Insert<'a>) -> Self {
        Query::Insert(Box::new(insert))
    }
}

impl<'a> From<SingleRowInsert<'a>> for Insert<'a> {
    fn from(insert: SingleRowInsert<'a>) -> Self {
        let values = if insert.values.is_empty() {
            Expression::from(Row::new())
        } else {
            Expression::from(insert.values)
        };

        Insert {
            table: insert.table,
            columns: insert.columns,
            values,
            on_conflict: None,
            returning: None,
            comment: None,
        }
    }
}

impl<'a> From<MultiRowInsert<'a>> for Insert<'a> {
    fn from(insert: MultiRowInsert<'a>) -> Self {
        let values = Expression::from(Values::new(insert.values));

        Insert {
            table: insert.table,
            columns: insert.columns,
            values,
            on_conflict: None,
            returning: None,
            comment: None,
        }
    }
}

impl<'a> From<SingleRowInsert<'a>> for Query<'a> {
    fn from(insert: SingleRowInsert<'a>) -> Query<'a> {
        Query::from(Insert::from(insert))
    }
}

impl<'a> From<MultiRowInsert<'a>> for Query<'a> {
    fn from(insert: MultiRowInsert<'a>) -> Query<'a> {
        Query::from(Insert::from(insert))
    }
}

impl<'a> Insert<'a> {
    /// Creates a new single row `INSERT` statement for the given table.
    ///
    /// ```rust
    /// # use grafbase_sql_ast::{ast::*, renderer::{Renderer, self}};
    /// # fn main() {
    /// let query = Insert::single_into("users");
    /// let (sql, _) = renderer::Postgres::build(query);
    ///
    /// assert_eq!(r#"INSERT INTO "users" DEFAULT VALUES"#, sql);
    /// # }
    /// ```
    pub fn single_into<T>(table: T) -> SingleRowInsert<'a>
    where
        T: Into<Table<'a>>,
    {
        SingleRowInsert {
            table: Some(table.into()),
            columns: Vec::new(),
            values: Row::new(),
        }
    }

    pub fn single() -> SingleRowInsert<'a> {
        SingleRowInsert {
            table: None,
            columns: Vec::new(),
            values: Row::new(),
        }
    }

    /// Creates a new multi row `INSERT` statement for the given table.
    pub fn multi_into<T, K, I>(table: T, columns: I) -> MultiRowInsert<'a>
    where
        T: Into<Table<'a>>,
        K: Into<Column<'a>>,
        I: IntoIterator<Item = K>,
    {
        MultiRowInsert {
            table: Some(table.into()),
            columns: columns.into_iter().map(|c| c.into()).collect(),
            values: Vec::new(),
        }
    }

    pub fn multi<K, I>(columns: I) -> MultiRowInsert<'a>
    where
        K: Into<Column<'a>>,
        I: IntoIterator<Item = K>,
    {
        MultiRowInsert {
            table: None,
            columns: columns.into_iter().map(|c| c.into()).collect(),
            values: Vec::new(),
        }
    }

    pub fn expression_into<T, I, K, E>(table: T, columns: I, expression: E) -> Self
    where
        T: Into<Table<'a>>,
        I: IntoIterator<Item = K>,
        K: Into<Column<'a>>,
        E: Into<Expression<'a>>,
    {
        Insert {
            table: Some(table.into()),
            columns: columns.into_iter().map(|c| c.into()).collect(),
            values: expression.into(),
            on_conflict: None,
            returning: None,
            comment: None,
        }
    }

    /// Sets the conflict resolution strategy.
    pub fn on_conflict(mut self, on_conflict: OnConflict<'a>) -> Self {
        self.on_conflict = Some(on_conflict);
        self
    }

    /// Adds a comment to the insert.
    pub fn comment<C: Into<Cow<'a, str>>>(mut self, comment: C) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Sets the returned columns.
    #[cfg(any(feature = "postgresql", feature = "mssql", feature = "sqlite"))]
    #[cfg_attr(
        feature = "docs",
        doc(cfg(any(feature = "postgresql", feature = "mssql", feature = "sqlite")))
    )]
    pub fn returning<K, I>(mut self, columns: I) -> Self
    where
        K: Into<Column<'a>>,
        I: IntoIterator<Item = K>,
    {
        self.returning = Some(columns.into_iter().map(|k| k.into()).collect());
        self
    }
}

impl<'a> SingleRowInsert<'a> {
    /// Adds a new value to the `INSERT` statement
    ///
    /// ```rust
    /// # use grafbase_sql_ast::{ast::*, renderer::{Renderer, self}};
    /// # fn main() {
    /// let query = Insert::single_into("users").value("foo", 10);
    /// let (sql, params) = renderer::Postgres::build(query);
    ///
    /// assert_eq!(r#"INSERT INTO "users" ("foo") VALUES ($1)"#, sql);
    /// assert_eq!(vec![Value::from(10)], params);
    /// # }
    /// ```
    pub fn value<K, V>(mut self, key: K, val: V) -> SingleRowInsert<'a>
    where
        K: Into<Column<'a>>,
        V: Into<Expression<'a>>,
    {
        self.columns.push(key.into());
        self.values.push(val.into());

        self
    }

    /// Convert into a common `Insert` statement.
    pub fn build(self) -> Insert<'a> {
        Insert::from(self)
    }
}

impl<'a> MultiRowInsert<'a> {
    /// Adds multiple new rows to be inserted.
    ///
    /// ```rust
    /// # use grafbase_sql_ast::{ast::*, renderer::{Renderer, self}};
    /// # fn main() {
    /// let query = Insert::multi_into("users", vec!["foo"])
    ///     .values(vec![1])
    ///     .values(vec![2]);
    ///
    /// let (sql, params) = renderer::Postgres::build(query);
    ///
    /// assert_eq!(r#"INSERT INTO "users" ("foo") VALUES ($1), ($2)"#, sql);
    ///
    /// assert_eq!(
    ///     vec![
    ///         Value::from(1),
    ///         Value::from(2),
    ///     ], params);
    /// # }
    /// ```
    pub fn values<V>(mut self, values: V) -> Self
    where
        V: Into<Row<'a>>,
    {
        self.values.push(values.into());
        self
    }

    /// Convert into a common `Insert` statement.
    pub fn build(self) -> Insert<'a> {
        Insert::from(self)
    }
}
