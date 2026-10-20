# Contributing to zreceipt

Thank you for your interest in improving zreceipt.

## Style
zreceipt follows the conventions of the Zcash Rust ecosystem. Use the
librustzcash style guides as the primary reference:
https://github.com/zcash/librustzcash/blob/main/CONTRIBUTING.md#styleguides

## Before opening a pull request
- Run `cargo fmt` and `cargo clippy`, and fix any warnings.
- Run `cargo test --release`; all tests must pass.
- Add tests for new behaviour, including test vectors for each pool
  (Sapling, Orchard, Ironwood) where relevant.

## Pull requests
- Keep each pull request focused on one change.
- Write clear commit messages: a short summary line, then the reason
  for the change.
- Describe what you changed and how you tested it.

## Security
Never commit viewing keys, spending keys or seed phrases, including in
tests or examples. Report security issues privately to the maintainer
instead of opening a public issue.

## License
By contributing, you agree that your contributions are licensed under
the same terms as the project (MIT).
