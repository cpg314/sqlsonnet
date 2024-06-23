use itertools::Itertools;
use pest::iterators::Pair;
use pest::Parser;

use crate::error::SQLParseError;
use crate::queries;

#[derive(pest_derive::Parser)]
#[grammar = "sql.pest"]
struct SQLParser;

pub(super) trait FromParsed: Sized {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError>;
}

impl FromParsed for queries::Query {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::query);
        queries::select::Query::parse(parsed.into_inner().next().unwrap())
            .map(queries::Query::Select)
    }
}

impl FromParsed for queries::Queries {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::queries);
        parsed
            .into_inner()
            .find_tagged("select")
            .map(|parsed| queries::select::Query::parse(parsed).map(queries::Query::Select))
            .collect::<Result<Vec<_>, _>>()
            .map(|r| r.into())
    }
}

// ASC or DESC
impl FromParsed for queries::order_by::Ordering {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::ordering);
        Ok(match parsed.as_str().to_lowercase().as_str() {
            "asc" => Self::Asc,
            "desc" => Self::Desc,
            _ => unreachable!(),
        })
    }
}
// A general expression
impl FromParsed for queries::Expr {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::expr);
        // Cleanup whitespace
        let parsed = parsed
            .as_str()
            .replace('\n', " ")
            .split(' ')
            .filter(|s| !s.is_empty())
            .join(" ");
        Ok(Self(parsed))
    }
}
// expr ASC or expr DESC
impl FromParsed for queries::order_by::Expr {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::order_expr);
        let mut parsed = parsed.into_inner();
        let expr = queries::Expr::parse(parsed.next().unwrap())?;
        let ordering = if let Some(parsed) = parsed.next() {
            queries::order_by::Ordering::parse(parsed)?
        } else {
            Default::default()
        };
        Ok(Self(expr, ordering))
    }
}

macro_rules! parse_sequence {
    ($tp: path, $rule: path, $map: path) => {
        impl FromParsed for $tp {
            fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
                assert_eq!(parsed.as_rule(), $rule);
                parsed
                    .into_inner()
                    .into_iter()
                    .map(|e| FromParsed::parse(e))
                    .collect::<Result<Vec<_>, _>>()
                    .map($map)
            }
        }
    };
}
// expr, expr, expr
parse_sequence!(queries::ExprList, Rule::exprs, Self);
// expr ASC, expr DESC
parse_sequence!(
    Vec<queries::order_by::Expr>,
    Rule::order_exprs,
    std::convert::identity
);

// FROM table
// FROM (subquery)
impl FromParsed for queries::from::From {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::table_or_subquery);
        let mut parsed = parsed.into_inner();

        let alias = parsed.find_first_tagged("as").map(|n| {
            assert_eq!(n.as_rule(), Rule::r#as);
            n.into_inner().nth(1).unwrap().as_str().to_string()
        });
        let n = parsed.next().unwrap();

        Ok(match n.as_rule() {
            Rule::select => {
                let mut select: queries::select::Query = queries::select::Query::parse(n)?;
                select.as_ = alias;
                Self::Subquery(Box::new(select))
            }
            Rule::identifier => Self::Table(n.as_str().into()).with_alias(alias),

            _ => {
                unreachable!()
            }
        })
    }
}
// ON a=b, c=d
// USING a, b
// USING (a, b)
impl FromParsed for queries::join::On {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::join_cond);
        let parsed = parsed.into_inner().next().unwrap();
        match parsed.as_rule() {
            Rule::using => {
                let parsed = parsed.into_inner().nth(1).unwrap();
                assert_eq!(parsed.as_rule(), Rule::identifiers);
                Ok(Self::Using(
                    parsed
                        .into_inner()
                        .map(|id| id.as_str().to_string())
                        .collect(),
                ))
            }
            Rule::on => Ok(Self::On(FromParsed::parse(
                parsed.into_inner().nth(1).unwrap(),
            )?)),
            _ => unreachable!(),
        }
    }
}
// JOIN table AS table2 ON/USING
// JOIN (subquery) ON/USING
impl FromParsed for queries::join::Join {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::join);
        let p = parsed.into_inner();
        let from = queries::from::From::parse(p.find_first_tagged("from").unwrap())?;
        let on = queries::join::On::parse(p.find_first_tagged("cond").unwrap())?;
        Ok(Self { from, on })
    }
}
impl FromParsed for queries::select::Query {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::select);
        let mut query = queries::select::Query::default();
        for p in parsed.into_inner() {
            let rule = p.as_rule();
            match rule {
                Rule::fields => {
                    query.fields = FromParsed::parse(p.into_inner().next().unwrap())?;
                }
                Rule::table_or_subquery => {
                    query.from = FromParsed::parse(p)?;
                }
                Rule::limit => {
                    query.limit = Some(
                        p.into_inner()
                            .find_first_tagged("limit")
                            .unwrap()
                            .as_str()
                            .parse()
                            .unwrap(),
                    );
                }
                Rule::order_by => {
                    query.order_by = FromParsed::parse(p.into_inner().nth(1).unwrap())?;
                }
                Rule::group_by => {
                    query.group_by = FromParsed::parse(p.into_inner().nth(1).unwrap())?;
                }
                Rule::join => {
                    query.joins.push(FromParsed::parse(p)?);
                }
                Rule::r#where => {
                    query.where_ = Some(FromParsed::parse(p.into_inner().nth(1).unwrap())?);
                }
                Rule::having => {
                    query.having = Some(FromParsed::parse(p.into_inner().nth(1).unwrap())?);
                }
                _ => {}
            }
        }
        Ok(query)
    }
}

pub(super) fn query_from_sql<T: FromParsed>(sql: &str, rule: Rule) -> Result<T, SQLParseError> {
    let parsed = SQLParser::parse(rule, sql)
        .map_err(|e| {
            let location = match e.location {
                pest::error::InputLocation::Pos(a) => a,
                pest::error::InputLocation::Span((a, _)) => a,
            };
            SQLParseError {
                reason: e.variant.message().into(),
                src: sql.into(),
                span: location.into(),
            }
        })?
        .next()
        .unwrap();
    T::parse(parsed)
}
