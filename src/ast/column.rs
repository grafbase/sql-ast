use super::Aliasable;
use crate::ast::{Expression, ExpressionKind, Table};
use std::borrow::Cow;

#[derive(Debug, Clone, Copy)]
pub enum TypeDataLength {
    Constant(u16),
    Maximum,
}

/// A column definition.
#[derive(Clone, Debug, Default)]
pub struct Column<'a> {
    pub name: Cow<'a, str>,
    pub(crate) table: Option<Table<'a>>,
    pub(crate) alias: Option<Cow<'a, str>>,
}

/// Defines a default value for a `Column`.
impl<'a> PartialEq for Column<'a> {
    fn eq(&self, other: &Column) -> bool {
        self.name == other.name && self.table == other.table
    }
}

impl<'a> Column<'a> {
    /// Create a bare version of the column, stripping out all other information
    /// other than the name.
    pub fn into_bare(self) -> Self {
        Self {
            name: self.name,
            ..Default::default()
        }
    }
}

impl<'a> From<Column<'a>> for Expression<'a> {
    fn from(col: Column<'a>) -> Self {
        Expression {
            kind: ExpressionKind::Column(Box::new(col)),
            alias: None,
        }
    }
}

impl<'a> Column<'a> {
    /// Create a column definition.
    pub fn new<S>(name: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        Column {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Include the table name in the column expression.
    pub fn table<T>(mut self, table: T) -> Self
    where
        T: Into<Table<'a>>,
    {
        self.table = Some(table.into());
        self
    }

    /// Include the table name in the column expression, if table is defined.
    pub fn opt_table<T>(mut self, table: Option<T>) -> Self
    where
        T: Into<Table<'a>>,
    {
        if let Some(table) = table {
            self.table = Some(table.into());
        }

        self
    }
}

impl<'a> Aliasable<'a> for Column<'a> {
    type Target = Column<'a>;

    fn alias<T>(mut self, alias: T) -> Self::Target
    where
        T: Into<Cow<'a, str>>,
    {
        self.alias = Some(alias.into());
        self
    }
}

impl<'a> From<&'a str> for Column<'a> {
    fn from(s: &'a str) -> Self {
        Column {
            name: s.into(),
            ..Default::default()
        }
    }
}

impl<'a, 'b> From<&'a &'b str> for Column<'b> {
    fn from(s: &'a &'b str) -> Self {
        Column::from(*s)
    }
}

impl<'a> From<String> for Column<'a> {
    fn from(s: String) -> Self {
        Column {
            name: s.into(),
            ..Default::default()
        }
    }
}

impl<'a, T, C> From<(T, C)> for Column<'a>
where
    T: Into<Table<'a>>,
    C: Into<Column<'a>>,
{
    fn from(t: (T, C)) -> Column<'a> {
        let mut column: Column<'a> = t.1.into();
        column = column.table(t.0);

        column
    }
}
