# `.gil` ACL inheritance on the published Windows tester preview — impact review (2026-10-18)

Scope: the limited unsigned tester channel `windows-preview-f0ecfbd` (default profile, text
Projects, storage format 4) and every Windows build before PR #25. This is a review of a
configuration-dependent risk, not a report of an observed exposure. No published artifact and no
user record is changed by this document.

## What the preview does

`gil start` on Windows creates `.gil` with `create_dir_all`, i.e. with the DACL inherited from
the folder the user selected. Everything inside inherits the same: `state.yaml` (interview
questions and answers, reports, contracts), `artifacts/blobs` (byte-for-byte copies of every
tracked project file at every snapshot, including contents of files the user later changed or
deleted), manifests, and recovery records. Monitor consent records under the user profile are
separate and already owner-only (`bindings_windows.rs`); they are not affected.

## Who can read or change those records

It depends only on the DACL the selected folder actually carries at the time `.gil` is created,
never on where the folder is. The location examples below describe Windows **defaults**; group
policy, a shared or domain-joined machine, a folder that was moved or copied, or any explicit
grant made earlier can change the effective DACL, so none of these rows is a guarantee:

| Project folder location (default Windows DACL as an example) | Typical inherited DACL | Other local standard account |
| --- | --- | --- |
| under `C:\Users\<user>\…` (Desktop, Documents, …) | user, SYSTEM, Administrators | usually no access; **not verified for any particular machine** |
| a folder created directly under `C:\` by a user | `Authenticated Users` Modify (subfolders and files), `Users` Read & execute | **read and modify** |
| `C:\Users\Public\…`, shared data folders, other volumes with permissive roots | typically `Everyone`/`Users` broad | **read**, often modify |
| OneDrive/junction/network paths | unsupported by the channel (refused or undefined) | n/a |

The risk is therefore deployment-conditional: it exists wherever the effective DACL grants
another account access, and it is absent only where the effective DACL does not. In the broad
cases the records are no more protected than the project files next to them, with one addition:
blobs keep past contents that the current files may no longer show. Administrators and SYSTEM
can read in every case; that matches the Unix root position.

## Is this a confirmed exposure?

No. There is no report, log or tester result showing another account reading a `.gil`. The
tester acceptance for this preview has not been performed (`TESTER-CHECK.md` is a draft), and
no installation count is available. It is a risk that exists only when a tester chose a folder
with a broad inherited DACL on a machine that has other local accounts.

## What PR #25 changes and what it does not

- New image Projects (format 5) created by the authoring profile get an explicit owner-only
  DACL (`spec/GIL_Windows_Visual_Support_Contract_v0.1.md` §8) and are refused on reopen if it
  was widened. Verified structurally and by actual cross-account access on the CI runner
  (contract §6 W6-1…W6-4).
- Text Projects (format 4) on Windows are **unchanged**: `gil start` still inherits. Nothing
  rewrites the ACL of an existing Project, format 4 or 5.
- The published preview bytes are unchanged.

## Options for the maintainer (none executed here)

1. Documentation only: tell testers that the preview's records inherit the selected folder's
   DACL, to check that DACL (for example with `icacls`) rather than rely on the folder's
   location, and to avoid folders whose DACL grants other accounts access (folders created
   directly under the system drive root and shared folders are the common cases). The published
   `INSTALL.md` is pinned to the release; updating it is a separate publication decision.
2. Next Windows preview: give the format-4 bootstrap (`lock::bootstrap`, non-Unix branch) the
   same owner-only DACL as format 5. This changes text-Project behavior and needs the same
   acceptance table; existing Projects would stay as they are.
3. A read-only diagnosis (for example in `gil status` on Windows) that says whether `.gil` grants
   access to accounts other than the owner and SYSTEM, without changing anything.
4. An explicit, separately approved adoption step for a user who wants to tighten an existing
   Project. The product currently has no such API (the earlier library function was removed on
   2026-10-19); a hand-made `icacls` DACL is accepted only when it matches the exact descriptor,
   which the CI fixture showed is not the case for a naive `(OI)(CI)F` grant.

Release classification: for the Windows **authoring** candidate the new-Project contract is
implemented and runner-verified; it is no longer an open item for that candidate. For the
existing **text** tester preview this remains an advisory until option 1 and/or 2 is decided.
