# gh-arm

**gh-arm** is a GitHub CLI extension to mark a pull request as ready for review and enable auto-merge.

## Prerequisites

- [GitHub CLI](https://cli.github.com/) installed and authenticated

## Install

```bash
gh extension install EricCrosson/gh-arm
```

## Use

```
gh arm [<pr>...] [flags]
```

With no arguments, arms the PR for the current branch.

Each `<pr>` is one of:

| Form                | Resolves against                    |
| ------------------- | ----------------------------------- |
| `<number>`          | PR number in the cwd repo           |
| `<branch>`          | Branch name in the cwd repo         |
| `-`                 | Previous git branch in the cwd repo |
| `OWNER/REPO#NUMBER` | Any repo — no cwd required          |

Multiple PRs are processed with try-all semantics: all are attempted even when some fail, and the command exits 1 if any failed.

### Flags

| Flag             | Description                                                      |
| ---------------- | ---------------------------------------------------------------- |
| `--dry-run`      | Print the `gh` invocations that would run without executing them |
| `-j, --jobs <N>` | Maximum concurrent PRs (default: 1, clamped to [1, 8])           |
| `-h, --help`     | Print usage                                                      |

> **Note:** bare refs (`<number>`, `<branch>`, `-`) and qualified atoms (`OWNER/REPO#NUMBER`) cannot be mixed in a single invocation.

### Examples

```bash
# Arm the PR for the current branch
gh arm

# Arm by PR number
gh arm 123

# Arm multiple PRs (try-all)
gh arm 123 456

# Arm a PR in another repo
gh arm EricCrosson/gh-arm#7

# Arm multiple cross-repo PRs in parallel
gh arm -j 4 org/repo-a#12 org/repo-b#34

# Preview what would run without executing
gh arm --dry-run 123
```

## License

Licensed under either:

- [MIT License](https://opensource.org/licenses/MIT)
- [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you shall be dual licensed as above, without
any additional terms or conditions.
