use serde_json::Value;

use crate::{ast::*, renderer::Renderer};
use std::fmt::{self, Write};

/// A visitor to generate queries for the PostgreSQL database.
///
/// The returned parameter values implement the `ToSql` trait from postgres and
/// can be used directly with the database.
#[cfg_attr(feature = "docs", doc(cfg(feature = "postgresql")))]
pub struct Postgres {
    query: String,
    parameters: Vec<Value>,
}

impl<'a> Renderer<'a> for Postgres {
    const C_BACKTICK_OPEN: &'static str = "\"";
    const C_BACKTICK_CLOSE: &'static str = "\"";
    const C_WILDCARD: &'static str = "%";

    fn build<Q>(query: Q) -> (String, Vec<Value>)
    where
        Q: Into<Query<'a>>,
    {
        let mut postgres = Postgres {
            query: String::with_capacity(4096),
            parameters: Vec::with_capacity(128),
        };

        Postgres::visit_query(&mut postgres, query.into());

        (postgres.query, postgres.parameters)
    }

    fn write<D: fmt::Display>(&mut self, s: D) {
        write!(&mut self.query, "{s}")
            .expect("we ran out of memory or something else why write failed");
    }

    fn add_parameter(&mut self, value: Value) {
        self.parameters.push(value);
    }

    fn parameter_substitution(&mut self) {
        self.write("$");
        self.write(self.parameters.len())
    }

    fn visit_limit_and_offset(&mut self, limit: Option<u32>, offset: Option<u32>) {
        match (limit, offset) {
            (Some(limit), Some(offset)) => {
                self.write(" LIMIT ");
                self.visit_parameterized(Value::from(limit));

                self.write(" OFFSET ");
                self.visit_parameterized(Value::from(offset))
            }
            (None, Some(offset)) => {
                self.write(" OFFSET ");
                self.visit_parameterized(Value::from(offset))
            }
            (Some(limit), None) => {
                self.write(" LIMIT ");
                self.visit_parameterized(Value::from(limit))
            }
            (None, None) => (),
        }
    }

    fn visit_insert(&mut self, insert: Insert<'a>) {
        self.write("INSERT ");

        if let Some(table) = insert.table.clone() {
            self.write("INTO ");
            self.visit_table(table, true);
        }

        match insert.values {
            Expression {
                kind: ExpressionKind::Row(row),
                ..
            } => {
                if row.values.is_empty() {
                    self.write(" DEFAULT VALUES");
                } else {
                    let columns = insert.columns.len();

                    self.write(" (");
                    for (i, c) in insert.columns.into_iter().enumerate() {
                        self.visit_column(c.name.into_owned().into());

                        if i < (columns - 1) {
                            self.write(",");
                        }
                    }

                    self.write(")");
                    self.write(" VALUES ");
                    self.visit_row(row);
                }
            }
            Expression {
                kind: ExpressionKind::Values(values),
                ..
            } => {
                let columns = insert.columns.len();

                self.write(" (");
                for (i, c) in insert.columns.into_iter().enumerate() {
                    self.visit_column(c.name.into_owned().into());

                    if i < (columns - 1) {
                        self.write(",");
                    }
                }

                self.write(")");
                self.write(" VALUES ");
                let values_len = values.len();

                for (i, row) in values.into_iter().enumerate() {
                    self.visit_row(row);

                    if i < (values_len - 1) {
                        self.write(", ");
                    }
                }
            }
            expr => self.surround_with("(", ")", |ref mut s| s.visit_expression(expr)),
        }

        match insert.on_conflict {
            Some(OnConflict::DoNothing) => self.write(" ON CONFLICT DO NOTHING"),
            Some(OnConflict::Update(update, constraints)) => {
                self.write(" ON CONFLICT");
                self.columns_to_bracket_list(constraints);
                self.write(" DO ");

                self.visit_upsert(update);
            }
            None => (),
        }

        if let Some(returning) = insert.returning {
            if !returning.is_empty() {
                let values = returning.into_iter().map(|r| r.into()).collect();
                self.write(" RETURNING ");
                self.visit_columns(values);
            }
        };
    }

    fn visit_aggregate_to_string(&mut self, value: Expression<'a>) {
        self.write("ARRAY_TO_STRING");
        self.write("(");
        self.write("ARRAY_AGG");
        self.write("(");
        self.visit_expression(value);
        self.write(")");
        self.write("','");
        self.write(")")
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

    fn visit_json_extract(&mut self, json_extract: JsonExtract<'a>) {
        match json_extract.path {
            #[cfg(feature = "mysql")]
            JsonPath::String(_) => {
                panic!("JSON path string notation is not supported for Postgres")
            }
            JsonPath::Array(json_path) => {
                self.write("(");
                self.visit_expression(*json_extract.column);

                if json_extract.extract_as_string {
                    self.write("#>>");
                } else {
                    self.write("#>");
                }

                // We use the `ARRAY[]::text[]` notation to better handle escaped character
                // The text protocol used when sending prepared statement doesn't seem to work well with escaped characters
                // when using the '{a, b, c}' string array notation.
                self.surround_with("ARRAY[", "]::text[]", |s| {
                    let len = json_path.len();
                    for (index, path) in json_path.into_iter().enumerate() {
                        s.visit_parameterized(Value::String(path.to_string()));
                        if index < len - 1 {
                            s.write(", ");
                        }
                    }
                });

                self.write(")");

                if !json_extract.extract_as_string {
                    self.write("::jsonb");
                }
            }
        }
    }

    fn visit_json_unquote(&mut self, json_unquote: JsonUnquote<'a>) {
        self.write("(");
        self.visit_expression(*json_unquote.expr);
        self.write("#>>ARRAY[]::text[]");
        self.write(")");
    }

    fn visit_array_contains(&mut self, left: Expression<'a>, right: Expression<'a>, not: bool) {
        if not {
            self.write("( NOT ");
        }

        self.visit_expression(left);
        self.write(" @> ");
        self.visit_expression(right);

        if not {
            self.write(" )");
        }
    }

    fn visit_array_contained(&mut self, left: Expression<'a>, right: Expression<'a>, not: bool) {
        if not {
            self.write("( NOT ");
        }

        self.visit_expression(left);
        self.write(" <@ ");
        self.visit_expression(right);

        if not {
            self.write(" )");
        }
    }

    fn visit_array_overlaps(&mut self, left: Expression<'a>, right: Expression<'a>) {
        self.visit_expression(left);
        self.write(" && ");
        self.visit_expression(right);
    }

    fn visit_json_extract_last_array_item(&mut self, extract: JsonExtractLastArrayElem<'a>) {
        self.write("(");
        self.visit_expression(*extract.expr);
        self.write("->-1");
        self.write(")");
    }

    fn visit_json_extract_first_array_item(&mut self, extract: JsonExtractFirstArrayElem<'a>) {
        self.write("(");
        self.visit_expression(*extract.expr);
        self.write("->0");
        self.write(")");
    }

    fn visit_json_type_equals(&mut self, left: Expression<'a>, json_type: JsonType<'a>, not: bool) {
        self.write("JSONB_TYPEOF");
        self.write("(");
        self.visit_expression(left);
        self.write(")");

        if not {
            self.write(" != ");
        } else {
            self.write(" = ");
        }

        match json_type {
            JsonType::Array => self.visit_expression(Value::String("array".to_string()).into()),
            JsonType::Boolean => self.visit_expression(Value::String("boolean".to_string()).into()),
            JsonType::Number => self.visit_expression(Value::String("number".to_string()).into()),
            JsonType::Object => self.visit_expression(Value::String("object".to_string()).into()),
            JsonType::String => self.visit_expression(Value::String("string".to_string()).into()),
            JsonType::Null => self.visit_expression(Value::String("null".to_string()).into()),
            JsonType::ColumnRef(column) => {
                self.write("JSONB_TYPEOF");
                self.write("(");
                self.visit_column(*column);
                self.write("::jsonb)")
            }
        }
    }

    fn visit_like(&mut self, left: Expression<'a>, right: Expression<'a>) {
        let need_cast = matches!(&left.kind, ExpressionKind::Column(_));
        self.visit_expression(left);

        // NOTE: Pg is strongly typed, LIKE comparisons are only between strings.
        // to avoid problems with types without implicit casting we explicitly cast to text
        if need_cast {
            self.write("::text");
        }

        self.write(" LIKE ");
        self.visit_expression(right);
    }

    fn visit_not_like(&mut self, left: Expression<'a>, right: Expression<'a>) {
        let need_cast = matches!(&left.kind, ExpressionKind::Column(_));
        self.visit_expression(left);

        // NOTE: Pg is strongly typed, LIKE comparisons are only between strings.
        // to avoid problems with types without implicit casting we explicitly cast to text
        if need_cast {
            self.write("::text");
        }

        self.write(" NOT LIKE ");
        self.visit_expression(right);
    }

    fn visit_ordering(&mut self, ordering: Ordering<'a>) {
        let len = ordering.0.len();

        for (i, (value, ordering)) in ordering.0.into_iter().enumerate() {
            let direction = ordering.map(|dir| match dir {
                Order::Asc => " ASC",
                Order::Desc => " DESC",
                Order::AscNullsFirst => "ASC NULLS FIRST",
                Order::AscNullsLast => "ASC NULLS LAST",
                Order::DescNullsFirst => "DESC NULLS FIRST",
                Order::DescNullsLast => "DESC NULLS LAST",
            });

            self.visit_expression(value);
            self.write(direction.unwrap_or(""));

            if i < (len - 1) {
                self.write(", ");
            }
        }
    }

    fn visit_concat(&mut self, concat: Concat<'a>) {
        let len = concat.exprs.len();

        self.surround_with("(", ")", |s| {
            for (i, expr) in concat.exprs.into_iter().enumerate() {
                s.visit_expression(expr);

                if i < (len - 1) {
                    s.write(" || ");
                }
            }
        });
    }

    fn visit_to_jsonb(&mut self, to_jsonb: ToJsonb<'a>) {
        self.write("to_jsonb(");
        self.visit_table(to_jsonb.table, false);
        self.write(".*)");
    }

    fn visit_json_agg(&mut self, json_agg: JsonAgg<'a>) {
        self.write("json_agg(");

        if json_agg.distinct {
            self.write("DISTINCT ");
        }

        self.visit_expression(json_agg.expression);

        if let Some(ordering) = json_agg.order_by {
            self.write(" ORDER BY ");
            self.visit_ordering(ordering);
        }

        self.write(")");
    }

    fn visit_encode(&mut self, encode: Encode<'a>) {
        self.write("encode(");
        self.visit_expression(encode.expression);
        self.write(", ");

        match encode.format {
            EncodeFormat::Base64 => self.write("'base64'"),
            EncodeFormat::Escape => self.write("'escape'"),
            EncodeFormat::Hex => self.write("'hex'"),
        }

        self.write(")");
    }

    fn visit_join_data(&mut self, data: JoinData<'a>) {
        if data.lateral {
            self.write(" LATERAL ");
        }

        self.visit_table(data.table, true);
        self.write(" ON ");
        self.visit_conditions(data.conditions)
    }
}

#[cfg(test)]
mod tests {
    use crate::renderer::*;

    fn expected_values<T>(sql: &'static str, params: Vec<T>) -> (String, Vec<Value>)
    where
        T: Into<Value>,
    {
        (
            String::from(sql),
            params.into_iter().map(|p| p.into()).collect(),
        )
    }

    fn default_params(mut additional: Vec<Value>) -> Vec<Value> {
        let mut result = Vec::new();

        for param in additional.drain(0..) {
            result.push(param)
        }

        result
    }

    #[test]
    fn test_single_row_insert_default_values() {
        let query = Insert::single_into("users");
        let (sql, params) = Postgres::build(query);

        assert_eq!("INSERT INTO \"users\" DEFAULT VALUES", sql);
        assert_eq!(default_params(vec![]), params);
    }

    #[test]
    fn test_single_row_insert() {
        let expected = expected_values("INSERT INTO \"users\" (\"foo\") VALUES ($1)", vec![10]);
        let query = Insert::single_into("users").value("foo", 10);
        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    #[cfg(feature = "postgresql")]
    fn test_returning_insert() {
        let expected = expected_values(
            "INSERT INTO \"users\" (\"foo\") VALUES ($1) RETURNING \"foo\"",
            vec![10],
        );
        let query = Insert::single_into("users").value("foo", 10);
        let (sql, params) = Postgres::build(Insert::from(query).returning(vec!["foo"]));

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    #[cfg(feature = "postgresql")]
    fn test_insert_on_conflict_update() {
        let expected = expected_values(
            "INSERT INTO \"users\" (\"foo\") VALUES ($1) ON CONFLICT (\"foo\") DO UPDATE SET \"foo\" = $2 WHERE \"users\".\"foo\" = $3 RETURNING \"foo\"",
            vec![10, 3, 1],
        );

        let update = Update::table("users")
            .set("foo", 3)
            .so_that(("users", "foo").equals(1));
        let query: Insert = Insert::single_into("users").value("foo", 10).into();
        let query = query.on_conflict(OnConflict::Update(update, Vec::from(["foo".into()])));
        let (sql, params) = Postgres::build(query.returning(vec!["foo"]));

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_multi_row_insert() {
        let expected = expected_values(
            "INSERT INTO \"users\" (\"foo\") VALUES ($1), ($2)",
            vec![10, 11],
        );

        let query = Insert::multi_into("users", vec!["foo"])
            .values(vec![10])
            .values(vec![11]);

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_limit_and_offset_when_both_are_set() {
        let expected = expected_values(
            "SELECT \"users\".* FROM \"users\" LIMIT $1 OFFSET $2",
            vec![10_i64, 2_i64],
        );

        let mut query = Select::from_table("users");
        query.limit(10);
        query.offset(2);

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_limit_and_offset_when_only_offset_is_set() {
        let expected = expected_values("SELECT \"users\".* FROM \"users\" OFFSET $1", vec![10_i64]);

        let mut query = Select::from_table("users");
        query.offset(10);

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_limit_and_offset_when_only_limit_is_set() {
        let expected = expected_values("SELECT \"users\".* FROM \"users\" LIMIT $1", vec![10_i64]);

        let mut query = Select::from_table("users");
        query.limit(10);

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_distinct() {
        let expected_sql = "SELECT DISTINCT \"bar\" FROM \"test\"";

        let mut query = Select::from_table("test");
        query.column(Column::new("bar"));
        query.distinct();

        let (sql, _) = Postgres::build(query);

        assert_eq!(expected_sql, sql);
    }

    #[test]
    fn test_distinct_with_subquery() {
        let expected_sql = "SELECT DISTINCT (SELECT $1 FROM \"test2\"), \"bar\" FROM \"test\"";

        let mut query = Select::from_table("test");

        query.value({
            let mut query = Select::from_table("test2");
            query.value(1);

            query
        });

        query.column(Column::new("bar"));
        query.distinct();

        let (sql, _) = Postgres::build(query);

        assert_eq!(expected_sql, sql);
    }

    #[test]
    fn test_from() {
        let expected_sql =
            "SELECT \"foo\".*, \"bar\".\"a\" FROM \"foo\", (SELECT \"a\" FROM \"baz\") AS \"bar\"";

        let mut query = Select::default();
        query.and_from("foo");

        query.and_from(
            Table::from({
                let mut query = Select::from_table("baz");
                query.column("a");
                query
            })
            .alias("bar"),
        );

        query.value(Table::from("foo").asterisk());
        query.column(("bar", "a"));

        let (sql, _) = Postgres::build(query);
        assert_eq!(expected_sql, sql);
    }

    #[test]
    fn test_like_cast_to_string() {
        let expected = expected_values(
            r#"SELECT "test".* FROM "test" WHERE "jsonField"::text LIKE $1"#,
            vec!["%foo%"],
        );

        let mut query = Select::from_table("test");
        query.so_that(Column::from("jsonField").like("%foo%"));

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_not_like_cast_to_string() {
        let expected = expected_values(
            r#"SELECT "test".* FROM "test" WHERE "jsonField"::text NOT LIKE $1"#,
            vec!["%foo%"],
        );

        let mut query = Select::from_table("test");
        query.so_that(Column::from("jsonField").not_like("%foo%"));

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_begins_with_cast_to_string() {
        let expected = expected_values(
            r#"SELECT "test".* FROM "test" WHERE "jsonField"::text LIKE $1"#,
            vec!["%foo"],
        );

        let mut query = Select::from_table("test");
        query.so_that(Column::from("jsonField").like("%foo"));

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_not_begins_with_cast_to_string() {
        let expected = expected_values(
            r#"SELECT "test".* FROM "test" WHERE "jsonField"::text NOT LIKE $1"#,
            vec!["%foo"],
        );

        let mut query = Select::from_table("test");
        query.so_that(Column::from("jsonField").not_like("%foo"));

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_ends_with_cast_to_string() {
        let expected = expected_values(
            r#"SELECT "test".* FROM "test" WHERE "jsonField"::text LIKE $1"#,
            vec!["foo%"],
        );

        let mut query = Select::from_table("test");
        query.so_that(Column::from("jsonField").like("foo%"));

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_not_ends_with_cast_to_string() {
        let expected = expected_values(
            r#"SELECT "test".* FROM "test" WHERE "jsonField"::text NOT LIKE $1"#,
            vec!["foo%"],
        );

        let mut query = Select::from_table("test");
        query.so_that(Column::from("jsonField").not_like("foo%"));

        let (sql, params) = Postgres::build(query);

        assert_eq!(expected.0, sql);
        assert_eq!(expected.1, params);
    }

    #[test]
    fn test_default_insert() {
        let insert = Insert::single_into("foo")
            .value("foo", "bar")
            .value("baz", default_value());

        let (sql, _) = Postgres::build(insert);

        assert_eq!(
            "INSERT INTO \"foo\" (\"foo\",\"baz\") VALUES ($1,DEFAULT)",
            sql
        );
    }

    #[test]
    fn join_is_inserted_positionally() {
        let joined_table = Table::from("User").left_join(
            Table::from("Post")
                .alias("p")
                .on(("p", "userId").equals(Column::from(("User", "id")))),
        );

        let mut q = Select::from_table(joined_table);
        q.and_from("Toto");

        let (sql, _) = Postgres::build(q);

        assert_eq!("SELECT \"User\".*, \"Toto\".* FROM \"User\" LEFT JOIN \"Post\" AS \"p\" ON \"p\".\"userId\" = \"User\".\"id\", \"Toto\"", sql);
    }
}
