#![allow(unstable_name_collisions)]
use serde_with::OneOrMany;
use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};

/// A set of SQL queries
#[serde_with::serde_as]
#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
pub struct Queries(#[serde_as(as = "OneOrMany<_>")] Vec<Query>);
impl Queries {
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl From<Vec<Query>> for Queries {
    fn from(source: Vec<Query>) -> Self {
        Self(source)
    }
}

impl IntoIterator for Queries {
    type Item = Query;

    type IntoIter = std::vec::IntoIter<Query>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
impl<'a> IntoIterator for &'a Queries {
    type Item = &'a Query;

    type IntoIter = std::slice::Iter<'a, Query>;

    fn into_iter(self) -> Self::IntoIter {
        let s = &self.0;
        s.iter()
    }
}

#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
pub struct Expr(pub String);
impl Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[serde_with::serde_as]
#[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
pub struct ExprList(#[serde_as(as = "OneOrMany<_>")] pub Vec<Expr>);
impl ExprList {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// An SQL query
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Query {
    Select(select::Query),
}

pub mod from {
    use super::*;
    #[derive(Deserialize, Serialize, Default, PartialEq, Eq, Debug)]
    #[serde(untagged)]
    pub enum From {
        Table(String),
        TableAlias {
            name: String,
            alias: String,
        },
        Subquery(Box<select::Query>),
        #[default]
        Unset,
    }
    impl From {
        pub fn with_alias(self, alias: Option<String>) -> Self {
            match (&self, alias) {
                (Self::Table(name), Some(alias)) => Self::TableAlias {
                    name: name.clone(),
                    alias,
                },
                (Self::TableAlias { name, .. }, Some(alias)) => Self::TableAlias {
                    name: name.clone(),
                    alias,
                },
                _ => self,
            }
        }
    }
}

pub mod join {
    use super::*;
    #[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
    #[serde(deny_unknown_fields)]
    pub struct Join {
        pub from: from::From,
        #[serde(flatten)]
        pub on: On,
    }
    #[serde_with::serde_as]
    #[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
    pub enum On {
        #[serde(rename = "on")]
        On(ExprList),
        #[serde(rename = "using")]
        Using(#[serde_as(as = "OneOrMany<_>")] Vec<String>),
    }
}

pub mod order_by {
    use super::*;
    #[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
    pub struct Expr(pub super::Expr, pub Ordering);
    #[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
    pub enum Ordering {
        #[default]
        #[serde(rename = "asc")]
        Asc,
        #[serde(rename = "desc")]
        Desc,
    }
}

pub mod select {
    use super::*;
    #[serde_with::serde_as]
    #[derive(Deserialize, Serialize, Debug, Default, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    pub struct Query {
        #[serde(skip_serializing_if = "ExprList::is_empty")]
        pub fields: ExprList,
        pub from: from::From,
        #[serde(rename = "where")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub where_: Option<Expr>,
        #[serde(rename = "groupBy")]
        #[serde(default, skip_serializing_if = "ExprList::is_empty")]
        pub group_by: ExprList,
        #[serde(default)]
        #[serde(skip_serializing_if = "Vec::is_empty")]
        #[serde_as(as = "OneOrMany<_>")]
        pub joins: Vec<join::Join>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub having: Option<Expr>,
        #[serde(default, rename = "orderBy", skip_serializing_if = "Vec::is_empty")]
        #[serde_as(as = "OneOrMany<_>")]
        pub order_by: Vec<order_by::Expr>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub limit: Option<usize>,
        #[serde(default, rename = "as")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub as_: Option<String>,
    }
}
