# Security policy

## Supported versions

Aphelion is pre-1.0. Only the latest release and the `main` branch receive
fixes.

| Version | Supported |
|---|---|
| `main` | ✅ |
| 0.1.x | ✅ |
| < 0.1 | ❌ |

## Threat model

Aphelion is a desktop simulator. It opens no network connections, runs no
sandboxed code and — as of 0.1 — does not load user-supplied files. Every
library crate is `#![forbid(unsafe_code)]`.

The realistic exposure is therefore:

- **Dependencies**, in particular `wgpu` and its GPU drivers, which do process
  untrusted-ish input in the form of shaders.
- **Scenario files**, once loading them lands (see the roadmap). A parser is an
  attack surface; it will be treated as one.

## Reporting a vulnerability

Please **do not** open a public issue for a security problem.

Use GitHub's private reporting instead:
[Security → Report a vulnerability](https://github.com/Pulsars-science/Aphelion/security/advisories/new).

Include what you can: the affected version, what an attacker gains, and steps to
reproduce.

You can expect an acknowledgement within a week and an assessment within two.
If a fix is warranted, we will agree a disclosure timeline with you and credit
you in the advisory unless you would rather we did not.

## Not security issues

- A simulation producing physically wrong results. That is a bug — please do
  report it as one, with the parameters and the energy drift.
- A crash from an extreme parameter value. Also a bug, also worth reporting.
- Poor performance or a GPU driver hang.
