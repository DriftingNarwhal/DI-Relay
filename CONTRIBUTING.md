# Contributing

## Licensing of contributions

This project is **licensed under the GNU Affero General Public License v3.0 (`LICENSE`)**. By contributing you agree your contribution is licensed
under those same terms.


## The gate

`cargo clippy --all-targets` must be clean before a change lands, and the binary must
build. This crate is deliberately thin — the relay itself is
`intranet_transport::RelayNode`, tested in the protocol repository — so behaviour changes
usually belong upstream rather than here.
