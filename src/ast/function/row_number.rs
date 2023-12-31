use crate::ast::{Column, IntoOrderDefinition, Over};

#[derive(Debug, Default, Clone, PartialEq)]
/// A window function that assigns a sequential integer
/// number to each row in the query’s result set.
pub struct RowNumber<'a> {
    pub(crate) over: Over<'a>,
}

impl<'a> RowNumber<'a> {
    /// Define the order of the row number. Is the row order if not set.
    pub fn order_by<T>(mut self, value: T) -> Self
    where
        T: IntoOrderDefinition<'a>,
    {
        self.over.ordering.append(value.into_order_definition());
        self
    }

    /// Define the partitioning of the row number
    pub fn partition_by<T>(mut self, partition: T) -> Self
    where
        T: Into<Column<'a>>,
    {
        self.over.partitioning.push(partition.into());
        self
    }
}

/// A number from 1 to n in specified order
pub fn row_number<'a>() -> RowNumber<'a> {
    RowNumber::default()
}
