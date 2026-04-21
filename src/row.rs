use crate::pr_ref::PrRef;

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: String,
    pub target: RowTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RowTarget {
    Pr(PrRef),
    CurrentBranch,
}

impl RowTarget {
    pub fn as_pref(&self) -> Option<&PrRef> {
        match self {
            RowTarget::Pr(pr) => Some(pr),
            RowTarget::CurrentBranch => None,
        }
    }
}

impl Row {
    pub fn current_branch() -> Self {
        Row {
            label: "current branch".into(),
            target: RowTarget::CurrentBranch,
        }
    }
}

impl From<PrRef> for Row {
    fn from(pr: PrRef) -> Self {
        let label = match &pr {
            PrRef::Bare(s) => s.clone(),
            PrRef::Qualified {
                owner,
                repo,
                number,
            } => format!("{owner}/{repo}#{number}"),
        };
        Row {
            label,
            target: RowTarget::Pr(pr),
        }
    }
}
