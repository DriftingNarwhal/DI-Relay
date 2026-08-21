# Contributing

## Licensing of contributions

This project is **licensed under the GNU Affero General Public License v3.0 (`LICENSE`)**. By contributing you agree your contribution is licensed
under those same terms.


## Sign your commits — the DCO

Every commit must carry a `Signed-off-by` line:

```
Signed-off-by: Your Name <your.email@example.com>
```

`git commit -s` adds it for you. Set `user.name` and `user.email` first, because the
line has to be a real name you can be reached at.

**Why this exists, and why it is worth the friction.** A copyleft licence is only as good
as the project's right to ship the code under it. Without a record that each contributor
had the right to contribute and agreed to the terms, that right is assumed rather than
established — and if this ever needs relicensing, or a licence term needs enforcing, the
answer is every past contributor's permission individually. The sign-off is a per-commit
statement making that explicit, and it is far cheaper now than reconstructed later.

It is deliberately **not** a copyright-assignment CLA. You keep your copyright; nothing is
signed over to anybody. What the sign-off certifies is the [Developer Certificate of
Origin](https://developercertificate.org/), reproduced in full below.

## The gate

`cargo clippy --all-targets` must be clean before a change lands, and the binary must
build. This crate is deliberately thin — the relay itself is
`intranet_transport::RelayNode`, tested in the protocol repository — so behaviour changes
usually belong upstream rather than here.

---

Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
