mod aggregate_to_string;
mod average;
mod coalesce;
mod concat;
mod count;
#[cfg(feature = "postgresql")]
mod encode;
#[cfg(feature = "postgresql")]
mod json_agg;
#[cfg(feature = "postgresql")]
mod json_build_object;
#[cfg(any(feature = "postgresql", feature = "mysql"))]
mod json_extract;
#[cfg(any(feature = "postgresql", feature = "mysql"))]
mod json_extract_array;
#[cfg(any(feature = "postgresql", feature = "mysql"))]
mod json_unquote;
mod lower;
mod maximum;
mod minimum;
mod row_number;
#[cfg(feature = "postgresql")]
mod row_to_json;
mod sum;
#[cfg(feature = "postgresql")]
mod to_jsonb;
mod upper;

pub use aggregate_to_string::*;
pub use average::*;
pub use coalesce::*;
pub use concat::*;
pub use count::*;
#[cfg(feature = "postgresql")]
pub use encode::*;
#[cfg(feature = "postgresql")]
pub use json_agg::*;
#[cfg(feature = "postgresql")]
pub use json_build_object::*;
#[cfg(any(feature = "postgresql", feature = "mysql"))]
pub use json_extract::*;
#[cfg(any(feature = "postgresql", feature = "mysql"))]
pub(crate) use json_extract_array::*;
#[cfg(any(feature = "postgresql", feature = "mysql"))]
pub use json_unquote::*;
pub use lower::*;
pub use maximum::*;
pub use minimum::*;
pub use row_number::*;
#[cfg(feature = "postgresql")]
pub use row_to_json::*;
pub use sum::*;
#[cfg(feature = "postgresql")]
pub use to_jsonb::*;
pub use upper::*;

use super::Aliasable;
use std::borrow::Cow;

/// A database function definition
#[derive(Debug, Clone, PartialEq)]
pub struct Function<'a> {
    pub(crate) typ_: FunctionType<'a>,
    pub(crate) alias: Option<Cow<'a, str>>,
}

impl<'a> Function<'a> {
    pub fn returns_json(&self) -> bool {
        match self.typ_ {
            #[cfg(feature = "postgresql")]
            FunctionType::RowToJson(_) => true,
            #[cfg(any(feature = "postgresql", feature = "mysql"))]
            FunctionType::JsonExtract(_) => true,
            #[cfg(any(feature = "postgresql", feature = "mysql"))]
            FunctionType::JsonExtractLastArrayElem(_) => true,
            #[cfg(any(feature = "postgresql", feature = "mysql"))]
            FunctionType::JsonExtractFirstArrayElem(_) => true,
            #[cfg(feature = "postgresql")]
            FunctionType::ToJsonb(_) => true,
            _ => false,
        }
    }
}

/// A database function type
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FunctionType<'a> {
    Count(Count<'a>),
    AggregateToString(AggregateToString<'a>),
    Average(Average<'a>),
    Sum(Sum<'a>),
    Lower(Lower<'a>),
    Upper(Upper<'a>),
    Minimum(Minimum<'a>),
    Maximum(Maximum<'a>),
    Coalesce(Coalesce<'a>),
    Concat(Concat<'a>),
    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    JsonExtract(JsonExtract<'a>),
    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    JsonExtractLastArrayElem(JsonExtractLastArrayElem<'a>),
    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    JsonExtractFirstArrayElem(JsonExtractFirstArrayElem<'a>),
    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    JsonUnquote(JsonUnquote<'a>),
    #[cfg(feature = "postgresql")]
    RowToJson(RowToJson<'a>),
    #[cfg(feature = "postgresql")]
    ToJsonb(ToJsonb<'a>),
    #[cfg(feature = "postgresql")]
    JsonAgg(JsonAgg<'a>),
    #[cfg(feature = "postgresql")]
    Encode(Encode<'a>),
    #[cfg(feature = "postgresql")]
    JsonBuildObject(JsonBuildObject<'a>),
}

impl<'a> Aliasable<'a> for Function<'a> {
    type Target = Function<'a>;

    fn alias<T>(mut self, alias: T) -> Self::Target
    where
        T: Into<Cow<'a, str>>,
    {
        self.alias = Some(alias.into());
        self
    }
}
