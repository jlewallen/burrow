#[derive(Debug, PartialEq, Clone)]
pub enum UnaryOperator {
    Minus,
    Not,
}

#[derive(Debug, PartialEq, Clone)]
pub enum BinaryOperator {
    Plus,
    Minus,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Expr {
    Bool(bool),
    Integer(i64),
    Real(f64),
    Variable(String),
    Option(Option<Box<Expr>>),
    FieldAccess {
        receiver: Box<Expr>,
        name: String,
    },
    UnaryExpr {
        op: UnaryOperator,
        child: Box<Expr>,
    },
    BinaryExpr {
        op: BinaryOperator,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::char,
    character::complete::{multispace0, one_of},
    combinator::recognize,
    combinator::{map, opt},
    multi::{many0, many1},
    sequence::{delimited, pair, preceded},
    sequence::{terminated, tuple},
    IResult,
};
use thiserror::Error;

fn decimal(i: &str) -> IResult<&str, &str> {
    recognize(many1(terminated(one_of("0123456789"), many0(char('_')))))(i)
}

// This was copied from the nom recipes and was the fastest solution I could
// find to being able to parse f64 and i64 separately.
fn float(i: &str) -> IResult<&str, &str> {
    alt((
        // Case one: .42
        recognize(tuple((
            char('.'),
            decimal,
            opt(tuple((one_of("eE"), opt(one_of("+-")), decimal))),
        ))), // Case two: 42e42 and 42.42e42
        recognize(tuple((
            decimal,
            opt(preceded(char('.'), decimal)),
            one_of("eE"),
            opt(one_of("+-")),
            decimal,
        ))), // Case three: 42. and 42.42
        recognize(tuple((decimal, char('.'), opt(decimal)))),
    ))(i)
}

fn parse_numeric_literal(i: &str) -> IResult<&str, Expr> {
    let float = map(float, |s: &str| {
        Expr::Real(s.parse::<f64>().expect("Error parsing Real"))
    });

    let integer = map(decimal, |s: &str| {
        Expr::Integer(s.parse::<i64>().expect("Error parsing Integer"))
    });

    alt((float, integer))(i)
}

fn parse_none_literal(i: &str) -> IResult<&str, Expr> {
    alt((map(alt((tag("none"), tag("None"))), |_| Expr::Option(None)),))(i)
}

fn parse_some_expr(i: &str) -> IResult<&str, Expr> {
    let some = alt((tag("some"), tag("Some")));
    let args = delimited(
        tag("("),
        map(parse_expr, |e| Expr::Option(Some(e.into()))),
        tag(")"),
    );

    preceded(some, args)(i)
}

fn parse_boolean_literal(i: &str) -> IResult<&str, Expr> {
    alt((
        map(tag("true"), |_| Expr::Bool(true)),
        map(tag("false"), |_| Expr::Bool(false)),
    ))(i)
}

fn variable_identifier(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| "_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
}

fn parse_variable(i: &str) -> IResult<&str, Expr> {
    let (i, initial) = map(variable_identifier, |id| Expr::Variable(id.to_owned()))(i)?;

    let (i, accesses) = nom::multi::many0(pair(one_of("."), variable_identifier))(i)?;

    Ok((
        i,
        accesses
            .into_iter()
            .fold(initial, |acc, (op, atom)| match op {
                '.' => Expr::FieldAccess {
                    receiver: acc.into(),
                    name: atom.to_owned(),
                },
                _ => unreachable!(),
            }),
    ))
}

fn parse_parenthesized(i: &str) -> IResult<&str, Expr> {
    delimited(
        preceded(multispace0, tag("(")),
        parse_expr,
        preceded(multispace0, tag(")")),
    )(i)
}

fn unary_operator(i: &str) -> IResult<&str, UnaryOperator> {
    map(one_of("-+!"), |v| match v {
        '-' => UnaryOperator::Minus,
        '!' => UnaryOperator::Not,
        _ => unimplemented!(),
    })(i)
}

fn parse_unary(i: &str) -> IResult<&str, Expr> {
    map(pair(opt(unary_operator), parse_atom), |(op, v)| match op {
        Some(op) => Expr::UnaryExpr {
            op,
            child: v.into(),
        },
        None => v,
    })(i)
}

fn parse_atom(i: &str) -> IResult<&str, Expr> {
    alt((
        parse_numeric_literal,
        parse_boolean_literal,
        parse_none_literal,
        parse_some_expr,
        parse_variable,
        parse_parenthesized,
    ))(i)
}

fn parse_expr(i: &str) -> IResult<&str, Expr> {
    let (i, initial) = parse_unary(i)?;
    fold_expression(initial, i)
}

fn binary_operator(i: &str) -> IResult<&str, BinaryOperator> {
    alt((
        map(tag(">="), |_| BinaryOperator::GreaterThanOrEqual),
        map(tag(">"), |_| BinaryOperator::GreaterThan),
        map(tag("<="), |_| BinaryOperator::LessThanOrEqual),
        map(tag("<"), |_| BinaryOperator::LessThan),
        map(tag("=="), |_| BinaryOperator::Equal),
        map(tag("!="), |_| BinaryOperator::NotEqual),
        map(one_of("+-*/"), |v| match v {
            '+' => BinaryOperator::Plus,
            '-' => BinaryOperator::Minus,
            '*' => BinaryOperator::Multiply,
            '/' => BinaryOperator::Divide,
            _ => unimplemented!(),
        }),
    ))(i)
}

fn fold_expression(initial: Expr, i: &str) -> IResult<&str, Expr> {
    let (i, operations) = nom::multi::many0(pair(
        preceded(multispace0, binary_operator),
        preceded(multispace0, parse_unary),
    ))(i)?;

    Ok((
        i,
        operations
            .into_iter()
            .fold(initial, |acc, (op, atom)| Expr::BinaryExpr {
                op,
                lhs: Box::new(acc),
                rhs: Box::new(atom),
            }),
    ))
}

pub fn parse(i: &str) -> Result<Expr, ParseError> {
    let (i, expr) = parse_expr(i)?;
    if !i.is_empty() {
        Err(ParseError::Unparsed(i.to_owned()))
    } else {
        Ok(expr)
    }
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unparsed text")]
    Unparsed(String),
    #[error("Parse failure")]
    Failure(String),
    #[error("Parse incomplete")]
    Incomplete(String),
    #[error("Parse error")]
    Error(String),
}

impl From<nom::Err<nom::error::Error<&str>>> for ParseError {
    fn from(source: nom::Err<nom::error::Error<&str>>) -> ParseError {
        match source {
            nom::Err::Incomplete(needed) => ParseError::Incomplete(format!("{:?}", needed)),
            nom::Err::Error(more) => ParseError::Error(more.to_string()),
            nom::Err::Failure(more) => ParseError::Failure(more.to_string()),
        }
    }
}
