# Opt-in Windows preview publication — f0ecfbd

The maintainer requested publication through a usable tester installation prompt after security
review. This record promotes the exact checked CI bytes to a limited unsigned tester channel;
the original CI receipts deliberately remain `publishable:false` historical build records.
No receipt is rewritten as if Windows GUI acceptance or signing had occurred.

Release tag: `windows-preview-f0ecfbd`, source `f0ecfbd1ddffd883a78fb82aba05c9716db084f6`.
Installation instructions and exact architecture-specific hashes: [INSTALL.md](INSTALL.md).
Both native jobs in run 37552996947 passed; downloads were independently extracted and compared
against the complete receipt inventory, PE architecture/system imports and private-path checks.
The payload has exactly eight regular files, no links, no dependency caches or user projects.
Known secret-key patterns and forbidden sensitive file names were checked, not an exhaustive
malware or secret-detection guarantee. Both npm audits reported zero findings; Rust audit scope
and remaining Companion-only warnings are recorded in [the security checkpoint](SECURITY-CHECK-20261007.md).

This is a GitHub downloadable local marketplace, not universal-directory listing or a stable
Windows support claim. The existing Mac releases/catalog/default branch are untouched.
No signing, ordinary-user Windows Codex installation or fullscreen acceptance is claimed.
The official CLI local marketplace/add interface was checked; host-specific end-to-end installation
is still the tester's acceptance task. If blocked, stop without security bypasses.

GIL source remains MIT. Bundled dependencies retain their complete notices; linked Microsoft
runtime code is not represented as GIL-owned or wholly MIT. Published Microsoft redistribution
conditions remain applicable. An individual GitHub support response is not a release permit;
the support closure did not determine rights either way. No vendor endorsement is claimed.

Publication completed: https://github.com/hyun06000/gil/releases/tag/windows-preview-f0ecfbd
The release is a prerelease and is not marked latest. Both tar.gz assets and INSTALL.md were
downloaded again without authentication over HTTPS; both archive hashes matched the fixed values
in INSTALL.md and the downloaded instructions matched the source byte-for-byte. No CI receipt
or previously released Mac bytes were changed. No end-user installation was performed here.
