//! Visitors for reading an abstract SQL syntax tree, generating the query and
//! gathering parameters in the right order.
//!
//! The visitor module should not know how to construct an AST, just how to read
//! one. Everything related to the tree generation is in the
//! [ast](../ast/index.html) module.
#[cfg(feature = "postgresql")]
mod postgres;

use serde_json::Value;

#[cfg(feature = "postgresql")]
pub use self::postgres::Postgres;

use crate::ast::*;
use std::fmt;

/// A function travelling through the query AST, building the final query string
/// and gathering parameters sent to the database together with the query.
pub trait Renderer<'a> {
    /// Opening backtick character to surround identifiers, such as column and table names.
    const C_BACKTICK_OPEN: &'static str;
    /// Closing backtick character to surround identifiers, such as column and table names.
    const C_BACKTICK_CLOSE: &'static str;
    /// Wildcard character to be used in `LIKE` queries.
    const C_WILDCARD: &'static str;

    /// Convert the given `Query` to an SQL string and a vector of parameters.
    /// When certain parameters are replaced with the `C_PARAM` character in the
    /// query, the vector should contain the parameter value in the right position.
    fn build<Q>(query: Q) -> (String, Vec<Value>)
    where
        Q: Into<Query<'a>>;

    /// Write to the query.
    fn write<D: fmt::Display>(&mut self, s: D);

    /// A point to modify an incoming query to make it compatible with the
    /// underlying database.
    fn compatibility_modifications(&self, query: Query<'a>) -> Query<'a> {
        query
    }

    fn surround_with<F>(&mut self, begin: &str, end: &str, f: F)
    where
        F: FnOnce(&mut Self),
    {
        self.write(begin);
        f(self);
        self.write(end)
    }

    fn columns_to_bracket_list(&mut self, columns: Vec<Column<'a>>) {
        let len = columns.len();

        self.write(" (");
        for (i, c) in columns.into_iter().enumerate() {
            self.visit_column(c.name.into_owned().into());

            if i < (len - 1) {
                self.write(",");
            }
        }
        self.write(")");
    }

    /// When called, the visitor decided to not render the parameter into the query,
    /// replacing it with the `C_PARAM`, calling `add_parameter` with the replaced value.
    fn add_parameter(&mut self, value: Value);

    /// The `LIMIT` and `OFFSET` statement in the query
    fn visit_limit_and_offset(&mut self, limit: Option<u32>, offset: Option<u32>);

    /// A visit in the `ORDER BY` section of the query
    fn visit_ordering(&mut self, ordering: Ordering<'a>);

    /// A walk through an `INSERT` statement
    fn visit_insert(&mut self, insert: Insert<'a>);

    /// What to use to substitute a parameter in the query.
    fn parameter_substitution(&mut self);

    /// What to use to substitute a parameter in the query.
    fn visit_aggregate_to_string(&mut self, value: Expression<'a>);

    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    fn visit_json_extract(&mut self, json_extract: JsonExtract<'a>);

    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    fn visit_json_extract_last_array_item(&mut self, extract: JsonExtractLastArrayElem<'a>);

    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    fn visit_json_extract_first_array_item(&mut self, extract: JsonExtractFirstArrayElem<'a>);

    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    fn visit_array_contains(&mut self, left: Expression<'a>, right: Expression<'a>, not: bool);

    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    fn visit_array_contained(&mut self, left: Expression<'a>, right: Expression<'a>, not: bool);

    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    fn visit_array_overlaps(&mut self, left: Expression<'a>, right: Expression<'a>);

    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    fn visit_json_type_equals(&mut self, left: Expression<'a>, right: JsonType<'a>, not: bool);

    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    fn visit_json_unquote(&mut self, json_unquote: JsonUnquote<'a>);

    #[cfg(feature = "postgresql")]
    fn visit_to_jsonb(&mut self, to_jsonb: ToJsonb<'a>);

    #[cfg(feature = "postgresql")]
    fn visit_json_build_object(&mut self, json_build_object: JsonBuildObject<'a>);

    #[cfg(feature = "postgresql")]
    fn visit_json_agg(&mut self, to_jsonb: JsonAgg<'a>);

    #[cfg(feature = "postgresql")]
    fn visit_encode(&mut self, encode: Encode<'a>);

    /// A walk through an `DELETE` statement
    fn visit_delete(&mut self, delete: Delete<'a>);

    /// A visit to a value we parameterize
    fn visit_parameterized(&mut self, value: Value) {
        self.add_parameter(value);
        self.parameter_substitution()
    }

    /// The join statements in the query
    fn visit_joins(&mut self, joins: Vec<Join<'a>>) {
        for j in joins {
            match j {
                Join::Inner(data) => {
                    self.write(" INNER JOIN ");
                    self.visit_join_data(data);
                }
                Join::Left(data) => {
                    self.write(" LEFT JOIN ");
                    self.visit_join_data(data);
                }
                Join::Right(data) => {
                    self.write(" RIGHT JOIN ");
                    self.visit_join_data(data);
                }
                Join::Full(data) => {
                    self.write(" FULL JOIN ");
                    self.visit_join_data(data);
                }
            }
        }
    }

    fn visit_join_data(&mut self, data: JoinData<'a>) {
        self.visit_table(data.table, true);
        self.write(" ON ");
        self.visit_conditions(data.conditions)
    }

    fn visit_common_table_expression(&mut self, cte: CommonTableExpression<'a>) {
        self.visit_table(Table::from(cte.name.into_owned()), false);
        self.write(" AS ");

        let query = cte.query;
        self.surround_with("(", ")", |ref mut s| s.visit_query(query));
    }

    /// A walk through a `SELECT` statement
    fn visit_select(&mut self, select: Select<'a>) {
        let number_of_ctes = select.ctes.len();

        if number_of_ctes > 0 {
            self.write("WITH ");

            for (i, cte) in select.ctes.into_iter().enumerate() {
                self.visit_common_table_expression(cte);

                if i < (number_of_ctes - 1) {
                    self.write(", ");
                }
            }

            self.write(" ");
        }

        self.write("SELECT ");

        if select.distinct {
            self.write("DISTINCT ");
        }

        if !select.tables.is_empty() {
            if select.columns.is_empty() {
                for (i, table) in select.tables.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }

                    match &table.typ {
                        TableType::Query(_) | TableType::Values(_) => match table.alias {
                            Some(ref alias) => {
                                self.surround_with(
                                    Self::C_BACKTICK_OPEN,
                                    Self::C_BACKTICK_CLOSE,
                                    |ref mut s| s.write(alias),
                                );
                                self.write(".*");
                            }
                            None => self.write("*"),
                        },
                        TableType::Table(_) => match table.alias.clone() {
                            Some(ref alias) => {
                                self.surround_with(
                                    Self::C_BACKTICK_OPEN,
                                    Self::C_BACKTICK_CLOSE,
                                    |ref mut s| s.write(alias),
                                );
                                self.write(".*");
                            }
                            None => {
                                self.visit_table(table.clone(), false);
                                self.write(".*");
                            }
                        },
                        TableType::JoinedTable(jt) => match table.alias.clone() {
                            Some(ref alias) => {
                                self.surround_with(
                                    Self::C_BACKTICK_OPEN,
                                    Self::C_BACKTICK_CLOSE,
                                    |ref mut s| s.write(alias),
                                );
                                self.write(".*");
                            }
                            None => {
                                let mut unjoined_table = table.clone();
                                // Convert the table typ to a `TableType::Table` for the SELECT statement print
                                // We only want the join to appear in the FROM clause
                                unjoined_table.typ = TableType::Table(jt.0.clone());

                                self.visit_table(unjoined_table, false);
                                self.write(".*");
                            }
                        },
                    }
                }
            } else {
                self.visit_columns(select.columns);
            }

            self.write(" FROM ");

            for (i, table) in select.tables.into_iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }

                self.visit_table(table, true);
            }

            if !select.joins.is_empty() {
                self.visit_joins(select.joins);
            }

            if let Some(conditions) = select.conditions {
                self.write(" WHERE ");
                self.visit_conditions(conditions);
            }
            if !select.grouping.is_empty() {
                self.write(" GROUP BY ");
                self.visit_grouping(select.grouping);
            }
            if let Some(conditions) = select.having {
                self.write(" HAVING ");
                self.visit_conditions(conditions);
            }
            if !select.ordering.is_empty() {
                self.write(" ORDER BY ");
                self.visit_ordering(select.ordering);
            }

            self.visit_limit_and_offset(select.limit, select.offset);
        } else if select.columns.is_empty() {
            self.write(" *");
        } else {
            self.visit_columns(select.columns);
        }
    }

    /// A walk through an `UPDATE` statement
    fn visit_update(&mut self, update: Update<'a>) {
        self.write("UPDATE ");
        self.visit_table(update.table, true);

        {
            self.write(" SET ");
            let pairs = update.columns.into_iter().zip(update.values);
            let len = pairs.len();

            for (i, (key, value)) in pairs.enumerate() {
                self.visit_column(key);
                self.write(" = ");
                self.visit_expression(value);

                if i < (len - 1) {
                    self.write(", ");
                }
            }
        }

        if let Some(conditions) = update.conditions {
            self.write(" WHERE ");
            self.visit_conditions(conditions);
        }

        if let Some(returning) = update.returning {
            if !returning.is_empty() {
                let values = returning.into_iter().map(|r| r.into()).collect();
                self.write(" RETURNING ");
                self.visit_columns(values);
            }
        }
    }

    fn visit_upsert(&mut self, update: Update<'a>) {
        self.write("UPDATE ");

        self.write("SET ");
        self.visit_update_set(update.clone());

        if let Some(conditions) = update.conditions {
            self.write(" WHERE ");
            self.visit_conditions(conditions);
        }
    }

    fn visit_update_set(&mut self, update: Update<'a>) {
        let pairs = update.columns.into_iter().zip(update.values);
        let len = pairs.len();

        for (i, (key, value)) in pairs.enumerate() {
            self.visit_column(key);
            self.write(" = ");
            self.visit_expression(value);

            if i < (len - 1) {
                self.write(", ");
            }
        }
    }

    /// A helper for delimiting an identifier, surrounding every part with `C_BACKTICK`
    /// and delimiting the values with a `.`
    fn delimited_identifiers(&mut self, parts: &[&str]) {
        let len = parts.len();

        for (i, part) in parts.iter().enumerate() {
            self.surround_with_backticks(part);

            if i < (len - 1) {
                self.write(".");
            }
        }
    }

    /// A helper for delimiting a part of an identifier, surrounding it with `C_BACKTICK`
    fn surround_with_backticks(&mut self, part: &str) {
        self.surround_with(
            Self::C_BACKTICK_OPEN,
            Self::C_BACKTICK_CLOSE,
            |ref mut s| s.write(part),
        );
    }

    /// A walk through a complete `Query` statement
    fn visit_query(&mut self, mut query: Query<'a>) {
        query = self.compatibility_modifications(query);

        match query {
            Query::Select(select) => self.visit_select(*select),
            Query::Insert(insert) => self.visit_insert(*insert),
            Query::Update(update) => self.visit_update(*update),
            Query::Delete(delete) => self.visit_delete(*delete),
        }
    }

    /// The selected columns
    fn visit_columns(&mut self, columns: Vec<Expression<'a>>) {
        let len = columns.len();

        for (i, column) in columns.into_iter().enumerate() {
            self.visit_expression(column);

            if i < (len - 1) {
                self.write(", ");
            }
        }
    }

    fn visit_operation(&mut self, op: SqlOp<'a>) {
        match op {
            SqlOp::Add(left, right) => self.surround_with("(", ")", |ref mut se| {
                se.visit_expression(left);
                se.write(" + ");
                se.visit_expression(right)
            }),
            SqlOp::Sub(left, right) => self.surround_with("(", ")", |ref mut se| {
                se.visit_expression(left);
                se.write(" - ");
                se.visit_expression(right)
            }),
            SqlOp::Mul(left, right) => self.surround_with("(", ")", |ref mut se| {
                se.visit_expression(left);
                se.write(" * ");
                se.visit_expression(right)
            }),
            SqlOp::Div(left, right) => self.surround_with("(", ")", |ref mut se| {
                se.visit_expression(left);
                se.write(" / ");
                se.visit_expression(right)
            }),
            SqlOp::Rem(left, right) => self.surround_with("(", ")", |ref mut se| {
                se.visit_expression(left);
                se.write(" % ");
                se.visit_expression(right)
            }),
            SqlOp::Append(left, right) => self.surround_with("(", ")", |ref mut se| {
                se.visit_expression(left);
                se.write(" || ");
                se.visit_expression(right)
            }),
            SqlOp::JsonDeleteAtPath(left, right) => self.surround_with("(", ")", |ref mut se| {
                se.visit_expression(left);
                se.write(" #- ");
                se.visit_expression(right);
            }),
        }
    }

    /// A visit to a value used in an expression
    fn visit_expression(&mut self, value: Expression<'a>) {
        match value.kind {
            ExpressionKind::Value(value) => self.visit_expression(*value),
            ExpressionKind::Raw(value) => self.write(value),
            ExpressionKind::ConditionTree(tree) => self.visit_conditions(tree),
            ExpressionKind::Compare(compare) => self.visit_compare(compare),
            ExpressionKind::Parameterized(val) => self.visit_parameterized(val),
            ExpressionKind::Column(column) => self.visit_column(*column),
            ExpressionKind::Row(row) => self.visit_row(row),
            ExpressionKind::Selection(selection) => {
                self.surround_with("(", ")", |ref mut s| s.visit_select(*selection))
            }
            ExpressionKind::Function(function) => self.visit_function(*function),
            ExpressionKind::Op(op) => self.visit_operation(*op),
            ExpressionKind::Values(values) => self.visit_values(values),
            ExpressionKind::Asterisk(table) => match table {
                Some(table) => {
                    self.visit_table(*table, false);
                    self.write(".*")
                }
                None => self.write("*"),
            },
            ExpressionKind::Default => self.write("DEFAULT"),
            ExpressionKind::Table(table) => self.visit_table(*table, false),
        }

        if let Some(alias) = value.alias {
            self.write(" AS ");

            self.delimited_identifiers(&[&*alias]);
        };
    }

    fn visit_multiple_tuple_comparison(&mut self, left: Row<'a>, right: Values<'a>, negate: bool) {
        self.visit_row(left);
        self.write(if negate { " NOT IN " } else { " IN " });
        self.visit_values(right)
    }

    fn visit_values(&mut self, values: Values<'a>) {
        self.surround_with("(", ")", |ref mut s| {
            let len = values.len();
            for (i, row) in values.into_iter().enumerate() {
                s.visit_row(row);

                if i < (len - 1) {
                    s.write(",");
                }
            }
        })
    }

    /// A database table identifier
    fn visit_table(&mut self, table: Table<'a>, include_alias: bool) {
        match table.typ {
            TableType::Table(table_name) => match table.database {
                Some(database) => self.delimited_identifiers(&[&*database, &*table_name]),
                None => self.delimited_identifiers(&[&*table_name]),
            },
            TableType::Values(values) => self.visit_values(values),
            TableType::Query(select) => {
                self.surround_with("(", ")", |ref mut s| s.visit_select(*select))
            }
            TableType::JoinedTable(jt) => {
                match table.database {
                    Some(database) => self.delimited_identifiers(&[&*database, &*jt.0]),
                    None => self.delimited_identifiers(&[&*jt.0]),
                }
                self.visit_joins(jt.1)
            }
        };

        if include_alias {
            if let Some(alias) = table.alias {
                self.write(" AS ");

                self.delimited_identifiers(&[&*alias]);
            };
        }
    }

    /// A database column identifier
    fn visit_column(&mut self, column: Column<'a>) {
        match column.table {
            Some(table) => {
                self.visit_table(table, false);
                self.write(".");
                self.delimited_identifiers(&[&*column.name]);
            }
            _ => self.delimited_identifiers(&[&*column.name]),
        };

        if let Some(alias) = column.alias {
            self.write(" AS ");
            self.delimited_identifiers(&[&*alias]);
        }
    }

    /// A row of data used as an expression
    fn visit_row(&mut self, row: Row<'a>) {
        self.surround_with("(", ")", |ref mut s| {
            let len = row.values.len();
            for (i, value) in row.values.into_iter().enumerate() {
                s.visit_expression(value);

                if i < (len - 1) {
                    s.write(",");
                }
            }
        })
    }

    /// A walk through the query conditions
    fn visit_conditions(&mut self, tree: ConditionTree<'a>) {
        match tree {
            ConditionTree::And(expressions) => self.surround_with("(", ")", |ref mut s| {
                let len = expressions.len();

                for (i, expr) in expressions.into_iter().enumerate() {
                    s.visit_expression(expr);

                    if i < (len - 1) {
                        s.write(" AND ");
                    }
                }
            }),
            ConditionTree::Or(expressions) => self.surround_with("(", ")", |ref mut s| {
                let len = expressions.len();

                for (i, expr) in expressions.into_iter().enumerate() {
                    s.visit_expression(expr);

                    if i < (len - 1) {
                        s.write(" OR ");
                    }
                }
            }),
            ConditionTree::Not(expression) => self.surround_with("(", ")", |ref mut s| {
                s.write("NOT ");
                s.visit_expression(*expression)
            }),
            ConditionTree::Single(expression) => self.visit_expression(*expression),
            ConditionTree::NoCondition => self.write("1=1"),
            ConditionTree::NegativeCondition => self.write("1=0"),
            ConditionTree::Exists(table) => self.surround_with("(", ")", |ref mut s| {
                s.write("EXISTS ");

                s.surround_with("(", ")", |ref mut s| {
                    s.visit_table(*table, false);
                })
            }),
        }
    }

    fn visit_greater_than(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" > ");
        self.visit_expression(right)
    }

    fn visit_greater_than_or_equals(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" >= ");
        self.visit_expression(right)
    }

    fn visit_less_than(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" < ");
        self.visit_expression(right)
    }

    fn visit_less_than_or_equals(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" <= ");
        self.visit_expression(right)
    }

    fn visit_like(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" LIKE ");
        self.visit_expression(right);
    }

    fn visit_not_like(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" NOT LIKE ");
        self.visit_expression(right);
    }

    /// A comparison expression
    fn visit_compare(&mut self, compare: Compare<'a>) {
        match compare {
            Compare::Equals(left, right) => self.visit_equals(*left, *right),
            Compare::NotEquals(left, right) => self.visit_not_equals(*left, *right),
            Compare::LessThan(left, right) => self.visit_less_than(*left, *right),
            Compare::LessThanOrEquals(left, right) => self.visit_less_than_or_equals(*left, *right),
            Compare::GreaterThan(left, right) => self.visit_greater_than(*left, *right),
            Compare::GreaterThanOrEquals(left, right) => {
                self.visit_greater_than_or_equals(*left, *right)
            }
            Compare::In(left, right) => match (*left, *right) {
                // To prevent `x IN ()` from happening.
                (
                    _,
                    Expression {
                        kind: ExpressionKind::Row(ref row),
                        ..
                    },
                ) if row.is_empty() => self.write("1=0"),

                // To prevent `x IN ()` from happening.
                (
                    Expression {
                        kind: ExpressionKind::Row(_),
                        ..
                    },
                    Expression {
                        kind: ExpressionKind::Values(ref vals),
                        ..
                    },
                ) if vals.row_len() == 0 => self.write("1=0"),

                // Flattening out a row.
                (
                    Expression {
                        kind: ExpressionKind::Row(mut cols),
                        ..
                    },
                    Expression {
                        kind: ExpressionKind::Values(vals),
                        ..
                    },
                ) if cols.len() == 1 && vals.row_len() == 1 => {
                    let col = cols.pop().unwrap();
                    let vals = vals.flatten_row().unwrap();

                    self.visit_expression(col);
                    self.write(" IN ");
                    self.visit_row(vals)
                }

                // No need to do `IN` if right side is only one value,
                (
                    left,
                    Expression {
                        kind: ExpressionKind::Parameterized(pv),
                        ..
                    },
                ) => {
                    self.visit_expression(left);
                    self.write(" = ");
                    self.visit_parameterized(pv)
                }

                (
                    Expression {
                        kind: ExpressionKind::Row(row),
                        ..
                    },
                    Expression {
                        kind: ExpressionKind::Values(values),
                        ..
                    },
                ) => self.visit_multiple_tuple_comparison(row, values, false),

                // expr IN (..)
                (left, right) => {
                    self.visit_expression(left);
                    self.write(" IN ");
                    self.visit_expression(right)
                }
            },
            Compare::NotIn(left, right) => match (*left, *right) {
                // To prevent `x NOT IN ()` from happening.
                (
                    _,
                    Expression {
                        kind: ExpressionKind::Row(ref row),
                        ..
                    },
                ) if row.is_empty() => self.write("1=1"),

                // To prevent `x NOT IN ()` from happening.
                (
                    Expression {
                        kind: ExpressionKind::Row(_),
                        ..
                    },
                    Expression {
                        kind: ExpressionKind::Values(ref vals),
                        ..
                    },
                ) if vals.row_len() == 0 => self.write("1=1"),

                // Flattening out a row.
                (
                    Expression {
                        kind: ExpressionKind::Row(mut cols),
                        ..
                    },
                    Expression {
                        kind: ExpressionKind::Values(vals),
                        ..
                    },
                ) if cols.len() == 1 && vals.row_len() == 1 => {
                    let col = cols.pop().unwrap();
                    let vals = vals.flatten_row().unwrap();

                    self.visit_expression(col);
                    self.write(" NOT IN ");
                    self.visit_row(vals)
                }

                // No need to do `IN` if right side is only one value,
                (
                    left,
                    Expression {
                        kind: ExpressionKind::Parameterized(pv),
                        ..
                    },
                ) => {
                    self.visit_expression(left);
                    self.write(" <> ");
                    self.visit_parameterized(pv)
                }

                (
                    Expression {
                        kind: ExpressionKind::Row(row),
                        ..
                    },
                    Expression {
                        kind: ExpressionKind::Values(values),
                        ..
                    },
                ) => self.visit_multiple_tuple_comparison(row, values, true),

                // expr IN (..)
                (left, right) => {
                    self.visit_expression(left);
                    self.write(" NOT IN ");
                    self.visit_expression(right)
                }
            },
            Compare::Like(left, right) => self.visit_like(*left, *right),
            Compare::NotLike(left, right) => self.visit_not_like(*left, *right),
            Compare::Null(column) => {
                self.visit_expression(*column);
                self.write(" IS NULL")
            }
            Compare::NotNull(column) => {
                self.visit_expression(*column);
                self.write(" IS NOT NULL")
            }
            Compare::Between(val, left, right) => {
                self.visit_expression(*val);
                self.write(" BETWEEN ");
                self.visit_expression(*left);
                self.write(" AND ");
                self.visit_expression(*right)
            }
            Compare::NotBetween(val, left, right) => {
                self.visit_expression(*val);
                self.write(" NOT BETWEEN ");
                self.visit_expression(*left);
                self.write(" AND ");
                self.visit_expression(*right)
            }
            Compare::Raw(left, comp, right) => {
                self.visit_expression(*left);
                self.write(" ");
                self.write(comp);
                self.write(" ");
                self.visit_expression(*right)
            }
            #[cfg(any(feature = "mysql", feature = "postgresql"))]
            Compare::JsonCompare(json_compare) => match json_compare {
                JsonCompare::ArrayContains(left, right) => {
                    self.visit_array_contains(*left, *right, false)
                }
                JsonCompare::ArrayContained(left, right) => {
                    self.visit_array_contained(*left, *right, false)
                }
                JsonCompare::ArrayOverlaps(left, right) => self.visit_array_overlaps(*left, *right),
                JsonCompare::ArrayNotContains(left, right) => {
                    self.visit_array_contains(*left, *right, true)
                }
                JsonCompare::TypeEquals(left, json_type) => {
                    self.visit_json_type_equals(*left, json_type, false)
                }
                JsonCompare::TypeNotEquals(left, json_type) => {
                    self.visit_json_type_equals(*left, json_type, true)
                }
            },
            #[cfg(feature = "postgresql")]
            Compare::Any(left) => {
                self.write("ANY");
                self.surround_with("(", ")", |s| s.visit_expression(*left))
            }
            #[cfg(feature = "postgresql")]
            Compare::All(left) => {
                self.write("ALL");
                self.surround_with("(", ")", |s| s.visit_expression(*left))
            }
        }
    }

    fn visit_equals(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" = ");
        self.visit_expression(right);
    }

    fn visit_not_equals(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" <> ");
        self.visit_expression(right);
    }

    /// A visit in the `GROUP BY` section of the query
    fn visit_grouping(&mut self, grouping: Grouping<'a>) {
        let len = grouping.0.len();

        for (i, value) in grouping.0.into_iter().enumerate() {
            self.visit_expression(value);

            if i < (len - 1) {
                self.write(", ");
            }
        }
    }

    fn visit_average(&mut self, avg: Average<'a>) {
        self.write("AVG");
        self.surround_with("(", ")", |ref mut s| s.visit_column(avg.column));
    }

    fn visit_function(&mut self, fun: Function<'a>) {
        match fun.typ_ {
            FunctionType::Count(fun_count) => {
                if fun_count.exprs.is_empty() {
                    self.write("COUNT(*)");
                } else {
                    self.write("COUNT");
                    self.surround_with("(", ")", |ref mut s| s.visit_columns(fun_count.exprs));
                }
            }
            FunctionType::AggregateToString(agg) => {
                self.visit_aggregate_to_string(agg.value.as_ref().clone());
            }
            #[cfg(feature = "postgresql")]
            FunctionType::RowToJson(row_to_json) => {
                self.write("ROW_TO_JSON");
                self.surround_with("(", ")", |ref mut s| s.visit_table(row_to_json.expr, false))
            }
            FunctionType::Average(avg) => {
                self.visit_average(avg);
            }
            FunctionType::Sum(sum) => {
                self.write("SUM");
                self.surround_with("(", ")", |ref mut s| s.visit_expression(*sum.expr));
            }
            FunctionType::Lower(lower) => {
                self.write("LOWER");
                self.surround_with("(", ")", |ref mut s| s.visit_expression(*lower.expression));
            }
            FunctionType::Upper(upper) => {
                self.write("UPPER");
                self.surround_with("(", ")", |ref mut s| s.visit_expression(*upper.expression));
            }
            FunctionType::Minimum(min) => {
                self.write("MIN");
                self.surround_with("(", ")", |ref mut s| s.visit_column(min.column));
            }
            FunctionType::Maximum(max) => {
                self.write("MAX");
                self.surround_with("(", ")", |ref mut s| s.visit_column(max.column));
            }
            FunctionType::Coalesce(coalesce) => {
                self.write("COALESCE");
                self.surround_with("(", ")", |s| s.visit_columns(coalesce.exprs));
            }
            #[cfg(any(feature = "postgresql", feature = "mysql"))]
            FunctionType::JsonExtract(json_extract) => {
                self.visit_json_extract(json_extract);
            }
            #[cfg(any(feature = "postgresql", feature = "mysql"))]
            FunctionType::JsonExtractFirstArrayElem(extract) => {
                self.visit_json_extract_first_array_item(extract);
            }
            #[cfg(any(feature = "postgresql", feature = "mysql"))]
            FunctionType::JsonExtractLastArrayElem(extract) => {
                self.visit_json_extract_last_array_item(extract);
            }
            #[cfg(any(feature = "postgresql", feature = "mysql"))]
            FunctionType::JsonUnquote(unquote) => {
                self.visit_json_unquote(unquote);
            }
            #[cfg(feature = "postgresql")]
            FunctionType::ToJsonb(to_jsonb) => self.visit_to_jsonb(to_jsonb),
            #[cfg(feature = "postgresql")]
            FunctionType::JsonAgg(json_agg) => self.visit_json_agg(json_agg),
            #[cfg(feature = "postgresql")]
            FunctionType::Encode(encode) => self.visit_encode(encode),
            #[cfg(feature = "postgresql")]
            FunctionType::JsonBuildObject(encode) => self.visit_json_build_object(encode),
            FunctionType::Concat(concat) => {
                self.visit_concat(concat);
            }
        };

        if let Some(alias) = fun.alias {
            self.write(" AS ");
            self.delimited_identifiers(&[&*alias]);
        }
    }

    fn visit_concat(&mut self, concat: Concat<'a>) {
        let len = concat.exprs.len();

        self.write("CONCAT");
        self.surround_with("(", ")", |s| {
            for (i, expr) in concat.exprs.into_iter().enumerate() {
                s.visit_expression(expr);

                if i < (len - 1) {
                    s.write(", ");
                }
            }
        });
    }

    fn visit_partitioning(&mut self, over: Over<'a>) {
        if !over.partitioning.is_empty() {
            let len = over.partitioning.len();
            self.write("PARTITION BY ");

            for (i, partition) in over.partitioning.into_iter().enumerate() {
                self.visit_column(partition);

                if i < (len - 1) {
                    self.write(", ");
                }
            }

            if !over.ordering.is_empty() {
                self.write(" ");
            }
        }

        if !over.ordering.is_empty() {
            self.write("ORDER BY ");
            self.visit_ordering(over.ordering);
        }
    }
}
