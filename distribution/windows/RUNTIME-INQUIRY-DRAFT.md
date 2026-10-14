# Hosted Windows build entitlement — inquiry record

This draft identifies the remaining question rather than claiming that a paid Visual Studio
subscription is required or that redistribution is prohibited. The available evidence does
not establish either conclusion. No terms have been accepted on the maintainer's behalf.

Suggested recipient: GitHub Support, with referral to Microsoft licensing support if necessary.

Subject: Redistributing an open-source executable built on standard GitHub-hosted Windows runners

We maintain the public MIT-licensed project https://github.com/hyun06000/gil.
Our standard GitHub-hosted windows-2025 and windows-11-arm runners build a Rust executable
with the preinstalled, non-preview Visual Studio Enterprise toolchain. The selected tools are
VS 18.10.12217.157, MSVC 14.51.36231 and Windows SDK 10.0.26100.0. The executable statically
links the CRT. We do not distribute Visual Studio, standalone .lib files or a runner image.
Third-party notices are preserved separately from the project's MIT license.

Please identify the applicable terms or official guidance establishing:

1. Whether our use of this preinstalled compiler through standard GitHub-hosted Actions includes
   the necessary build entitlement without a separate maintainer Visual Studio subscription.
2. Which terms cover redistribution of our resulting executable, including linked release
   libcmt/libvcruntime and Windows SDK libucrt code.
3. Whether any additional recipient notice or agreement is required for an unsigned, opt-in
   test build distributed without charge.

Build evidence: https://github.com/hyun06000/gil/actions/runs/37542348090.
We are asking about resulting application binaries, not redistribution of the hosted image.

## Status

Submitted to GitHub Support with maintainer approval on 2026-10-07. GitHub closed the inquiry
because the account/request is served through self-service resources. It did not answer the
licensing questions. This is neither redistribution permission nor a prohibition. Private support
account information is not included here. No automatic repost or reopening is authorized.

The maintainer subsequently clarified that an individual Support response is not a release
prerequisite. Use applicable published terms and preserve required notices; focus preview gates
on concrete security, integrity and installation checks. Do not claim legal certification.

Official references examined:
- https://learn.microsoft.com/en-us/visualstudio/releases/2026/redistribution
- https://www.microsoft.com/licensing/guidance/Visual-Studio
- https://visualstudio.microsoft.com/license-terms/vs2026-ga-pro-enterprise/
