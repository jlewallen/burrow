pub enum Ast {}

pub enum UnaryOperator {
    Plus,
    Minus,
}

pub enum Operator {
    Plus,
    Minus,
    Mul,
    Div,
}

pub enum Node {
    Integer(i64),
    Real(f64),
    UnaryExpr {
        op: UnaryOperator,
        child: Box<Node>,
    },
    BinaryExpr {
        op: Operator,
        lhs: Box<Node>,
        rhs: Box<Node>,
    },
}
