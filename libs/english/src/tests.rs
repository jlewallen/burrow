use super::*;

struct Fixture {
    text: &'static str,
    expected: Vec<English>,
}

impl Fixture {
    pub fn new(text: &'static str, expected: Vec<English>) -> Self {
        Self { text, expected }
    }
}

fn get_fixtures() -> Vec<Fixture> {
    vec![
        Fixture::new(
            r#"HOLD #unheld"#,
            vec![English::Literal("HOLD".into()), English::Unheld],
        ),
        Fixture::new(
            r#"PUT #held IN #held"#,
            vec![
                English::Literal("PUT".into()),
                English::Held,
                English::Literal("IN".into()),
                English::Held,
            ],
        ),
        Fixture::new(
            r#"PUT #held (INSIDE OF|IN) (#held|#unheld)?"#,
            vec![
                English::Literal("PUT".into()),
                English::Held,
                English::OneOf(vec![
                    English::Phrase(vec![
                        English::Literal("INSIDE".into()),
                        English::Literal("OF".into()),
                    ]),
                    English::Phrase(vec![English::Literal("IN".into())]),
                ]),
                English::Optional(Box::new(English::OneOf(vec![
                    English::Phrase(vec![English::Held]),
                    English::Phrase(vec![English::Unheld]),
                ]))),
            ],
        ),
        Fixture::new(
            r#"TAKE (OUT)? #contained (OUT OF (#held|#unheld))?"#,
            vec![
                English::Literal("TAKE".into()),
                English::Optional(Box::new(English::Phrase(vec![English::Literal(
                    "OUT".into(),
                )]))),
                English::Contained,
                English::Optional(Box::new(English::Phrase(vec![
                    English::Literal("OUT".into()),
                    English::Literal("OF".into()),
                    English::OneOf(vec![
                        English::Phrase(vec![English::Held]),
                        English::Phrase(vec![English::Unheld]),
                    ]),
                ]))),
            ],
        ),
        Fixture::new(
            r#"HOLD #unheld"#,
            vec![English::Literal("HOLD".into()), English::Unheld],
        ),
        Fixture::new(
            r#"DROP (#held)?"#,
            vec![
                English::Literal("DROP".into()),
                English::Optional(Box::new(English::Phrase(vec![English::Held]))),
            ],
        ),
        Fixture::new(
            r#"EDIT #3493"#,
            vec![English::Literal("EDIT".into()), English::Numbered(3493)],
        ),
        Fixture::new(
            r#"DIG #text TO #text FOR #text"#,
            vec![
                English::Literal("DIG".into()),
                English::Text,
                English::Literal("TO".into()),
                English::Text,
                English::Literal("FOR".into()),
                English::Text,
            ],
        ),
        Fixture::new(
            r#"MAKE ITEM #text"#,
            vec![
                English::Literal("MAKE".into()),
                English::Literal("ITEM".into()),
                English::Text,
            ],
        ),
    ]
}

#[test]
fn should_parse_all_english_fixtures() {
    for fixture in get_fixtures() {
        let (remaining, actual) = to_english(fixture.text).unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, fixture.expected);
    }
}
