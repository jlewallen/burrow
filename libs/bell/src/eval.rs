use std::{
    collections::HashMap,
    ops::{Add, Div, Mul, Neg, Not, Sub},
};

use thiserror::Error;

use crate::parse::{BinaryOperator, Expr, UnaryOperator};

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Real(f64),
    Option(Option<Box<Value>>),
    Map(HashMap<String, Value>),
}

impl Value {
    fn try_field(&self, key: &str) -> Value {
        match self {
            Value::Null => todo!(),
            Value::Bool(_) => todo!(),
            Value::Integer(_) => todo!(),
            Value::Real(_) => todo!(),
            Value::Option(_) => todo!(),
            Value::Map(map) => match map.get(key) {
                Some(value) => value.clone(),
                None => Value::Option(None),
            },
        }
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Real(value)
    }
}

impl TryInto<bool> for Value {
    type Error = EvaluationError;

    fn try_into(self) -> Result<bool, Self::Error> {
        match self {
            Value::Bool(value) => Ok(value),
            _ => unimplemented!(),
        }
    }
}

impl Add for Value {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => (lhs + rhs).into(),
            (Value::Integer(lhs), Value::Real(rhs)) => (lhs as f64 + rhs).into(),
            (Value::Real(lhs), Value::Integer(rhs)) => (lhs + rhs as f64).into(),
            (Value::Real(lhs), Value::Real(rhs)) => (lhs + rhs).into(),
            _ => unimplemented!(),
        }
    }
}

impl Sub for Value {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => (lhs - rhs).into(),
            (Value::Integer(lhs), Value::Real(rhs)) => (lhs as f64 - rhs).into(),
            (Value::Real(lhs), Value::Integer(rhs)) => (lhs - rhs as f64).into(),
            (Value::Real(lhs), Value::Real(rhs)) => (lhs - rhs).into(),
            _ => unimplemented!(),
        }
    }
}

impl Div for Value {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => (lhs / rhs).into(),
            (Value::Integer(lhs), Value::Real(rhs)) => (lhs as f64 / rhs).into(),
            (Value::Real(lhs), Value::Integer(rhs)) => (lhs / rhs as f64).into(),
            (Value::Real(lhs), Value::Real(rhs)) => (lhs / rhs).into(),
            _ => unimplemented!(),
        }
    }
}

impl Mul for Value {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => (lhs * rhs).into(),
            (Value::Integer(lhs), Value::Real(rhs)) => (lhs as f64 * rhs).into(),
            (Value::Real(lhs), Value::Integer(rhs)) => (lhs * rhs as f64).into(),
            (Value::Real(lhs), Value::Real(rhs)) => (lhs * rhs).into(),
            _ => unimplemented!(),
        }
    }
}

impl Neg for Value {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Value::Null => todo!(),
            Value::Bool(_) => todo!(),
            Value::Integer(value) => (-value).into(),
            Value::Real(value) => (-value).into(),
            Value::Option(_) => todo!(),
            Value::Map(_) => todo!(),
        }
    }
}

impl Not for Value {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Value::Null => todo!(),
            Value::Bool(value) => (!value).into(),
            Value::Integer(_) => todo!(),
            Value::Real(_) => todo!(),
            Value::Option(_) => todo!(),
            Value::Map(_) => todo!(),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(l0), Self::Bool(r0)) => l0 == r0,
            (Self::Integer(l0), Self::Integer(r0)) => l0 == r0,
            (Self::Real(l0), Self::Real(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Bool(lhs), Value::Bool(rhs)) => lhs.partial_cmp(rhs),
            (Value::Integer(lhs), Value::Integer(rhs)) => lhs.partial_cmp(rhs),
            (Value::Integer(lhs), Value::Real(rhs)) => (*lhs as f64).partial_cmp(rhs),
            (Value::Real(lhs), Value::Integer(rhs)) => lhs.partial_cmp(&(*rhs as f64)),
            (Value::Real(lhs), Value::Real(rhs)) => lhs.partial_cmp(rhs),
            _ => unimplemented!(),
        }
    }
}

/*
impl Index<&str> for Value {
    type Output = Value;

    fn index(&self, index: &str) -> &Self::Output {
        match self {
            Value::Null => todo!(),
            Value::Bool(_) => todo!(),
            Value::Integer(_) => todo!(),
            Value::Real(_) => todo!(),
            Value::Option(_) => todo!(),
            Value::Map(map) => match map.get(index) {
                Some(value) => &Self::Option(Some(value.clone().into())),
                None => &Self::Option(None),
            },
        }
    }
}
*/

pub struct Evaluator<'a> {
    scope: &'a HashMap<String, Value>,
}

impl<'a> Evaluator<'a> {
    pub fn new(scope: &'a HashMap<String, Value>) -> Self {
        Evaluator { scope }
    }

    pub fn eval(&self, expr: &Expr) -> Result<Value, EvaluationError> {
        match expr {
            Expr::Bool(val) => Ok(Value::Bool(*val)),
            Expr::Integer(val) => Ok(Value::Integer(*val)),
            Expr::Real(val) => Ok(Value::Real(*val)),
            Expr::Variable(name) => {
                if let Some(var) = self.scope.get(name) {
                    Ok(var.clone())
                } else {
                    Err(EvaluationError::NotFound(name.to_owned()))
                }
            }
            Expr::Option(child) => match child
                .as_ref()
                .map(|c| self.eval(c))
                .map_or(Ok(None), |v| v.map(Some))?
            {
                Some(child) => Ok(Value::Option(Some(child.into()))),
                None => Ok(Value::Option(None)),
            },

            Expr::FieldAccess { receiver, name } => {
                let receiver = self.eval(&receiver)?;
                Ok(receiver.try_field(name))
            }
            Expr::UnaryExpr { op, child } => match op {
                UnaryOperator::Minus => Ok(-self.eval(child)?),
                UnaryOperator::Not => Ok(!self.eval(child)?),
            },
            Expr::BinaryExpr { op, lhs, rhs } => match op {
                BinaryOperator::Plus => Ok(self.eval(lhs)? + self.eval(rhs)?),
                BinaryOperator::Minus => Ok(self.eval(lhs)? - self.eval(rhs)?),
                BinaryOperator::Multiply => Ok(self.eval(lhs)? * self.eval(rhs)?),
                BinaryOperator::Divide => Ok(self.eval(lhs)? / self.eval(rhs)?),
                BinaryOperator::Equal => Ok((self.eval(lhs)? == self.eval(rhs)?).into()),
                BinaryOperator::NotEqual => Ok((self.eval(lhs)? != self.eval(rhs)?).into()),
                BinaryOperator::Or => {
                    Ok((self.eval(lhs)?.try_into()? || self.eval(rhs)?.try_into()?).into())
                }
                BinaryOperator::And => {
                    Ok((self.eval(lhs)?.try_into()? && self.eval(rhs)?.try_into()?).into())
                }
                BinaryOperator::GreaterThan => Ok((self.eval(lhs)? > self.eval(rhs)?).into()),
                BinaryOperator::LessThan => Ok((self.eval(lhs)? < self.eval(rhs)?).into()),
                BinaryOperator::GreaterThanOrEqual => {
                    Ok((self.eval(lhs)? >= self.eval(rhs)?).into())
                }
                BinaryOperator::LessThanOrEqual => Ok((self.eval(lhs)? <= self.eval(rhs)?).into()),
            },
        }
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum EvaluationError {
    #[error("Invalid field")]
    InvalidField(String),
    #[error("Invalid cast")]
    InvalidCast,
    #[error("Not found {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::eval::{EvaluationError, Evaluator, Value};

    #[test]
    fn test_evaluate_literals() {
        let scope = HashMap::default();
        let evaluator = Evaluator::new(&scope);

        let tree = crate::parse::parse("1").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Integer(1));
        let tree = crate::parse::parse("true").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("3.14159").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(3.14159));
    }

    #[test]
    fn test_evaluate_boolean_operations() {
        let scope = HashMap::default();
        let evaluator = Evaluator::new(&scope);

        let tree = crate::parse::parse("true != true").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
        let tree = crate::parse::parse("true == true").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("true != false").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("true == !false").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("true || false").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("true && false").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
        let tree = crate::parse::parse("true or false").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("true and false").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_evaluate_integer_operations() {
        let scope = HashMap::default();
        let evaluator = Evaluator::new(&scope);

        let tree = crate::parse::parse("1 + 1").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Integer(2));
        let tree = crate::parse::parse("1 - 1").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Integer(0));
        let tree = crate::parse::parse("3 * 2").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Integer(6));
        let tree = crate::parse::parse("4 / 2").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Integer(2));

        let tree = crate::parse::parse("3 > 2").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("3 < 2").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
        let tree = crate::parse::parse("3 <= 3").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("3 >= 3").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("-3 >= 3").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_evaluate_real_operations() {
        let scope = HashMap::default();
        let evaluator = Evaluator::new(&scope);

        let tree = crate::parse::parse("1.0 + 1.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(2.0));
        let tree = crate::parse::parse("1.0 - 1.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(0.0));
        let tree = crate::parse::parse("3.0 * 2.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(6.0));
        let tree = crate::parse::parse("4.0 / 2.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(2.0));

        let tree = crate::parse::parse("3.0 > 2.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("3.0 < 2.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
        let tree = crate::parse::parse("3.0 <= 3.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("3.0 >= 3.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("-3.0 >= 3.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_evaluate_mixed_operations() {
        let scope = HashMap::default();
        let evaluator = Evaluator::new(&scope);

        let tree = crate::parse::parse("1 + 1.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(2.0));
        let tree = crate::parse::parse("1.0 - 1").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(0.0));
        let tree = crate::parse::parse("3 * 2.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(6.0));
        let tree = crate::parse::parse("4.0 / 2").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Real(2.0));

        let tree = crate::parse::parse("3 > 2.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("3.0 < 2").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
        let tree = crate::parse::parse("3 <= 3.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("3.0 >= 3").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(true));
        let tree = crate::parse::parse("-3 >= 3.0").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_simple_variables() {
        let scope: HashMap<_, _> = [("health".to_owned(), Value::Integer(100))].into();
        let evaluator = Evaluator::new(&scope);

        let tree = crate::parse::parse("health").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Integer(100));

        let tree = crate::parse::parse("healt").unwrap();
        assert_eq!(
            evaluator.eval(&tree).err().unwrap(),
            EvaluationError::NotFound("healt".to_owned())
        );
    }

    #[test]
    fn test_index_hash_map() {
        let child: HashMap<_, _> = [("health".to_owned(), Value::Integer(100))].into();
        let scope: HashMap<_, _> = [("person".to_owned(), Value::Map(child))].into();
        let evaluator = Evaluator::new(&scope);

        let tree = crate::parse::parse("person.health").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Integer(100));

        let tree = crate::parse::parse("person.healt").unwrap();
        assert_eq!(evaluator.eval(&tree).unwrap(), Value::Option(None));
    }

    /*
    #[test]
    fn test_index_json() {
        let child = json!({
            "health": 100
        });
        let scope: HashMap<_, _> = [("person".to_owned(), Value::Json(child))].into();
        let evaluator = Evaluator::new(&scope);
    }
    */
}
