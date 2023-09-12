use super::Function;
use crate::ast::Expression;

#[derive(Debug, Clone, PartialEq)]
/// A representation of the `json_agg` function in PostgreSQL.
pub struct JsonAgg<'a> {
    pub(crate) expression: Expression<'a>,
    pub(crate) distinct: bool,
}

/// Return the given table as JSONB collection.
pub fn json_agg<'a>(expression: impl Into<Expression<'a>>, distinct: bool) -> Function<'a> {
    let fun = JsonAgg {
        expression: expression.into(),
        distinct,
    };

    fun.into()
}

impl<'a> From<JsonAgg<'a>> for Function<'a> {
    fn from(value: JsonAgg<'a>) -> Self {
        Self {
            typ_: super::FunctionType::JsonAgg(value),
            alias: None,
        }
    }
}
