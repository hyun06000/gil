# Security reporting

GitHub Private Vulnerability Reporting is enabled for
[hyun06000/gil](https://github.com/hyun06000/gil) and the
[distribution marketplace](https://github.com/hyun06000/gil-marketplace).
The setting and repository owner's administrative access were verified on 2026-09-30.

**[Report a vulnerability privately](https://github.com/hyun06000/gil/security/advisories/new).**
A GitHub account is required. Use the repository's Security → Advisories → Report a vulnerability
entry if the direct form does not open. Do not post vulnerability details in a public issue or PR.

## Scope

The current user channels are the unsigned previews listed in [INSTALL.md](INSTALL.md): macOS Apple
Silicon (Plugin/archive) and Windows x64 (Companion installer), each used from Codex or from the Claude
Code tab of Claude Desktop. Neither is Apple Developer ID signed/notarized or Authenticode signed.
Old preview versions, other platforms, other Claude surfaces (terminal Claude Code, Chat, Cowork) and
review-only Companion builds do not have a promised maintenance window. Reports about current source or the distribution route are also welcome;
accepting a report does not promise a fix or support for every version.

The current Artifact scope excludes only the root `.gil/` directory. It does not honor
`.gitignore`, and restore can replace or delete any other regular file, including `.git/`, secrets,
dependencies and build output. Do not use restore on a valuable existing working tree. A path or
restore operation that can affect data outside the selected project is in security scope.

Include the version, affected component, expected and actual behavior, impact and the smallest safe
reproduction you can provide. Prefer synthetic projects. Never include credentials, private keys,
personal conversations, complete real `.gil` records or unrelated personal paths. A screenshot or
real project is not required. Do not test against other users' data or third-party services.

The repository owner [@hyun06000](https://github.com/hyun06000) is responsible for triage. There is no
guaranteed response deadline, fix schedule, paid support or bounty. Delivery of a notification email
has not been tested; an enabled reporting setting is not proof of an email being delivered.
No fake vulnerability report was submitted for validation.

This private channel is for security vulnerabilities, not ordinary UI bugs or conduct complaints.
See [SUPPORT](SUPPORT.md) and [community conduct](CODE_OF_CONDUCT.md) for those paths.
If the private form is unavailable, do not fall back to publishing sensitive details.
