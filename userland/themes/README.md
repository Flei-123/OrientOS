# `userland/themes/` — the colour schemes OrientOS ships

Each file is one colour scheme in the shape Osum's `wlibc.theme_load`
reads (`kernel/user/wlibc.fi`): one `key=rrggbb` per line, `#` starts a
comment, and unknown keys are counted as bad rather than ignored
silently. The eighteen keys are the ones `theme_names()` registers, no
more and no less.

`pkg/bauen.sh` wraps every file here into an **asset package**
(`kind=asset`, `class=theme`) so that a colour scheme travels the same
way a program does: content-addressed, in the store, in the signed
`INDEX`, collected when nothing names it. A `PLAN` then carries the
scheme as `pref <user> theme <sha256>` — a decision that names content,
which is the rule in [docs/PLAN-FORMAT.md](../../docs/PLAN-FORMAT.md).

A colour scheme is a **user** preference, not a system one: two people on
one machine want different ones. It renders to
`users/<who>/config/desktop/theme`, and only on a single-account machine
is it additionally visible as `/etc/theme`, which is the path Osum's
taskbar and desktop still read today.
