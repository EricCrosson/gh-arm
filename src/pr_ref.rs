#[derive(Debug, Clone, PartialEq)]
pub enum PrRef {
    Bare(String),
    Qualified {
        owner: String,
        repo: String,
        number: String,
    },
}

#[derive(Debug)]
pub enum ParseError {
    MalformedAtom(String),
    MixedRefs { bare: String, qualified: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::MalformedAtom(arg) => write!(
                f,
                "error: '{arg}': qualified atoms must be OWNER/REPO#NUMBER with a positive integer"
            ),
            ParseError::MixedRefs { bare, qualified } => write!(
                f,
                "error: cannot mix bare PR refs and qualified atoms in one invocation\n  bare:      {bare}\n  qualified: {qualified}"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

pub fn parse_arg(arg: &str) -> Result<PrRef, ParseError> {
    // If arg contains '#' and the segment before '#' contains '/', treat as
    // an attempted qualified atom and validate strictly.
    if let Some(hash_pos) = arg.rfind('#') {
        let before_hash = &arg[..hash_pos];
        if before_hash.contains('/') {
            let slash_pos = before_hash.find('/').unwrap();
            let owner = &before_hash[..slash_pos];
            let repo = &before_hash[slash_pos + 1..];
            let number = &arg[hash_pos + 1..];

            if owner.is_empty() || repo.is_empty() || repo.contains('/') {
                return Err(ParseError::MalformedAtom(arg.to_string()));
            }
            if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
                return Err(ParseError::MalformedAtom(arg.to_string()));
            }
            return Ok(PrRef::Qualified {
                owner: owner.to_string(),
                repo: repo.to_string(),
                number: number.to_string(),
            });
        }
    }
    Ok(PrRef::Bare(arg.to_string()))
}

pub fn validate_homogeneous(refs: &[PrRef]) -> Result<(), ParseError> {
    let mut first_bare: Option<String> = None;
    let mut first_qualified: Option<String> = None;

    for pr_ref in refs {
        match pr_ref {
            PrRef::Bare(s) => {
                first_bare.get_or_insert_with(|| s.clone());
            }
            PrRef::Qualified {
                owner,
                repo,
                number,
            } => {
                first_qualified.get_or_insert_with(|| format!("{owner}/{repo}#{number}"));
            }
        }
    }

    if let (Some(bare), Some(qualified)) = (first_bare, first_qualified) {
        return Err(ParseError::MixedRefs { bare, qualified });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atom_accepts() {
        assert!(matches!(
            parse_arg("BitGo/foo#123"),
            Ok(PrRef::Qualified { ref owner, ref repo, ref number })
            if owner == "BitGo" && repo == "foo" && number == "123"
        ));
        assert!(matches!(
            parse_arg("owner/repo-with-dash#1"),
            Ok(PrRef::Qualified { .. })
        ));
        assert!(matches!(
            parse_arg("o/r.with.dots#42"),
            Ok(PrRef::Qualified { .. })
        ));
    }

    #[test]
    fn bare_accepts() {
        assert_eq!(parse_arg("123").unwrap(), PrRef::Bare("123".into()));
        assert_eq!(
            parse_arg("feature-branch").unwrap(),
            PrRef::Bare("feature-branch".into())
        );
        assert_eq!(
            parse_arg("owner/repo").unwrap(),
            PrRef::Bare("owner/repo".into())
        );
    }

    #[test]
    fn malformed_atom_rejects() {
        let cases = [
            "owner/repo#abc",
            "owner/repo#",
            "owner//repo#1",
            "/repo#1",
            "owner/repo#1a",
            "a/b/c#1",
        ];
        for arg in &cases {
            assert!(
                matches!(parse_arg(arg), Err(ParseError::MalformedAtom(_))),
                "expected MalformedAtom for {arg:?}"
            );
        }
    }

    #[test]
    fn homogeneity_all_bare_ok() {
        let refs = vec![PrRef::Bare("1".into()), PrRef::Bare("2".into())];
        assert!(validate_homogeneous(&refs).is_ok());
    }

    #[test]
    fn homogeneity_all_qualified_ok() {
        let refs = vec![
            PrRef::Qualified {
                owner: "a".into(),
                repo: "b".into(),
                number: "1".into(),
            },
            PrRef::Qualified {
                owner: "c".into(),
                repo: "d".into(),
                number: "2".into(),
            },
        ];
        assert!(validate_homogeneous(&refs).is_ok());
    }

    #[test]
    fn homogeneity_mixed_errors() {
        let refs = vec![
            PrRef::Bare("123".into()),
            PrRef::Qualified {
                owner: "BitGo".into(),
                repo: "foo".into(),
                number: "456".into(),
            },
        ];
        match validate_homogeneous(&refs).unwrap_err() {
            ParseError::MixedRefs { bare, qualified } => {
                assert_eq!(bare, "123");
                assert_eq!(qualified, "BitGo/foo#456");
            }
            e => panic!("expected MixedRefs, got {e:?}"),
        }
    }

    #[test]
    fn homogeneity_empty_ok() {
        assert!(validate_homogeneous(&[]).is_ok());
    }
}
