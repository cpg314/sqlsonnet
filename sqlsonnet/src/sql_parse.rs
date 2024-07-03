use pest::iterators::Pair;
use pest::Parser;

use crate::error::SQLParseError;
use crate::queries;

#[derive(pest_derive::Parser)]
#[grammar = "sql.pest"]
struct SQLParser;

#[allow(dead_code)]
fn show_pair(pair: Pair<Rule>, indent: usize) {
    for _ in 0..indent {
        print!("  ",);
    }
    println!("{:?}", pair.as_rule());
    for inner in pair.into_inner() {
        show_pair(inner, indent + 1);
    }
}

pub(super) trait FromParsed: Sized {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError>;
}

impl FromParsed for queries::Query {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        let query = match parsed.as_rule() {
            Rule::select => parsed,
            Rule::query => parsed.into_inner().next().unwrap(),
            _ => unreachable!(),
        };
        FromParsed::parse(query).map(Self::Select)
    }
}

impl FromParsed for queries::Queries {
    fn parse(parsed: Pair<Rule>) -> Result<Self, SQLParseError> {
        assert_eq!(parsed.as_rule(), Rule::queries);
        parsed
            .into_inner()
            .filter(|p| p.as_rule() == Rule::select)
            .map(|parsed| queries::Query::parse(parsed))
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
        if !parsed
            .clone()
            .into_inner()
            .flatten()
            .any(|p| p.as_rule() == Rule::select)
        {
            return Ok(parsed.as_str().into());
        }
        const SEQ: bool = false;
        match parsed.as_rule() {
            Rule::expr => {
                let mut parsed = parsed.into_inner();
                let mut term1 = Self::parse(parsed.next().unwrap())?;
                let mut v: Vec<(queries::Operator, Box<Self>)> = vec![];
                let mut alias = None;
                for parsed in parsed {
                    match parsed.as_rule() {
                        Rule::op_term => {
                            let mut parsed = parsed.into_inner();
                            let op = queries::Operator(parsed.next().unwrap().as_str().into());
                            let term = Self::parse(parsed.next().unwrap())?;
                            if SEQ {
                                v.push((op, Box::new(term)));
                            } else {
                                term1 = term1.operator(op, term);
                            }
                        }
                        Rule::r#as => {
                            let id = parsed
                                .into_inner()
                                .find_first_tagged("id")
                                .unwrap()
                                .as_str();
                            alias = Some(id.to_string());
                        }
                        _ => unreachable!(),
                    }
                }
                if SEQ && !v.is_empty() {
                    term1 = Self::OperatorSeq(Box::new(term1), v)
                }
                if let Some(alias) = alias {
                    term1 = Self::Aliased {
                        expr: Box::new(term1),
                        alias,
                    }
                }
                Ok(term1)
            }
            Rule::term => {
                let term = parsed.as_str();
                let parsed = parsed.into_inner();
                if let Some(parsed) = parsed.find_first_tagged("select") {
                    let select = queries::Query::parse(parsed)?;
                    return Ok(Self::Subquery(Box::new(select)));
                }
                Ok(term.into())
            }
            _ => unreachable!(),
        }
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
        Ok(Self::new(expr, ordering))
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

        let from = match n.as_rule() {
            Rule::select => {
                let query = queries::select::Query::parse(n)?;
                Self::Subquery {
                    query: Box::new(query),
                    alias,
                }
            }
            Rule::identifier => Self::Table(n.as_str().into()).with_alias(alias),

            _ => {
                unreachable!()
            }
        };
        Ok(from)
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
                    query.from = Some(FromParsed::parse(p)?);
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
                src: miette::NamedSource::new("source.sql", sql.into()),
                span: location.into(),
            }
        })?
        .next()
        .unwrap();
    T::parse(parsed)
}
