use crate::parse::*;

#[test]
pub fn test_empty_expression_is_error() {
    assert!(parse("").is_err());
}

#[test]
pub fn test_numeric_literals() {
    assert_eq!(parse("0").unwrap(), Expr::Integer(0));
    assert_eq!(parse("1").unwrap(), Expr::Integer(1));
    assert_eq!(parse("100").unwrap(), Expr::Integer(100));
    assert_eq!(parse("3.14159").unwrap(), Expr::Real(3.14159));
}

#[test]
pub fn test_binary_expressions() {
    let one = Expr::Integer(1);
    let two = Expr::Integer(2);

    assert_eq!(
        parse("1 + 2").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::Plus,
            lhs: one.clone().into(),
            rhs: two.clone().into()
        }
    );
    assert_eq!(
        parse("1 - 2").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::Minus,
            lhs: one.clone().into(),
            rhs: two.clone().into()
        }
    );
    assert_eq!(
        parse("1 * 2").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::Multiply,
            lhs: one.clone().into(),
            rhs: two.clone().into()
        }
    );
    assert_eq!(
        parse("2 / 1").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::Divide,
            lhs: two.clone().into(),
            rhs: one.clone().into()
        }
    );
}

#[test]
pub fn test_negated_numeric_literals() {
    assert_eq!(
        parse("-100").unwrap(),
        Expr::UnaryExpr {
            op: UnaryOperator::Minus,
            child: Expr::Integer(100).into()
        }
    );
    assert_eq!(
        parse("-3.14159").unwrap(),
        Expr::UnaryExpr {
            op: UnaryOperator::Minus,
            child: Expr::Real(3.14159).into()
        }
    );
}

#[test]
pub fn test_identifiers() {
    assert_eq!(
        parse("carrying").unwrap(),
        Expr::Variable("carrying".to_owned())
    );
    assert_eq!(parse("_foo").unwrap(), Expr::Variable("_foo".to_owned()));
    assert_eq!(
        parse("this_way").unwrap(),
        Expr::Variable("this_way".to_owned())
    );
    assert_eq!(
        parse("ewGross").unwrap(),
        Expr::Variable("ewGross".to_owned())
    );
}

#[test]
pub fn test_field_accesses() {
    assert_eq!(
        parse("carrying.containing").unwrap(),
        Expr::FieldAccess {
            receiver: Expr::Variable("carrying".to_owned()).into(),
            name: "containing".to_owned()
        }
    );
}

#[test]
pub fn test_logical_operators() {
    let one = Expr::Integer(1);
    let two = Expr::Integer(2);

    assert_eq!(
        parse("1 > 2").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::GreaterThan,
            lhs: one.clone().into(),
            rhs: two.clone().into()
        }
    );
    assert_eq!(
        parse("1 < 2").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::LessThan,
            lhs: one.clone().into(),
            rhs: two.clone().into()
        }
    );
    assert_eq!(
        parse("1 >= 2").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::GreaterThanOrEqual,
            lhs: one.clone().into(),
            rhs: two.clone().into()
        }
    );
    assert_eq!(
        parse("2 <= 1").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::LessThanOrEqual,
            lhs: two.clone().into(),
            rhs: one.clone().into()
        }
    );
    assert_eq!(
        parse("2 == 1").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::Equal,
            lhs: two.clone().into(),
            rhs: one.clone().into()
        }
    );
    assert_eq!(
        parse("2 != 1").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::NotEqual,
            lhs: two.clone().into(),
            rhs: one.clone().into()
        }
    );
}

#[test]
pub fn test_complex_expressions() {
    let one = Expr::Integer(1);
    let two = Expr::Integer(2);

    assert_eq!(
        parse("(1 + 2) / 4").unwrap(),
        Expr::BinaryExpr {
            op: BinaryOperator::Divide,
            lhs: Expr::BinaryExpr {
                op: BinaryOperator::Plus,
                lhs: one.clone().into(),
                rhs: two.clone().into(),
            }
            .into(),
            rhs: Expr::Integer(4).into()
        }
    );

    assert_eq!(
        parse("!(1 == 2)").unwrap(),
        Expr::UnaryExpr {
            op: UnaryOperator::Not,
            child: Expr::BinaryExpr {
                op: BinaryOperator::Equal,
                lhs: one.into(),
                rhs: two.into(),
            }
            .into(),
        }
    );
}
