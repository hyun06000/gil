# GIL for Linux (x86-64, glibc) — per-user install

This archive holds one executable, `bin/gil`: the GIL Core, the MCP server Agents connect to and
the GIL Monitor, with every currently approved feature. There is no reduced or benchmark-only
edition, and nothing else needs to be downloaded. Node, Cargo and a source checkout are not used
at run time.

It is an **unsigned development build**. `release.json` in this folder lists the SHA-256 of every
file, the source it was built from and the glibc version the executable requires
(`elf.glibc_required`). That value is read from the binary; a list of supported distributions is
not stated by this build.

## Install

Check the archive against its published `SHA256SUMS`, extract it to a new folder, then run:

```sh
./bin/gil install
```

- The executable is copied to `~/.local/share/gil/bin/gil` together with `install.json`, which
  records its version and digest. No administrator rights are used and no security setting is
  changed.
- If something else already exists at that path and was not placed by `gil install` (no
  `install.json` beside it that records exactly that file — even when its bytes are the same), or
  if `~/.local/share/gil` or its `bin` folder is a link or not your own folder, the install stops
  and changes nothing, Host settings included. Move that entry away yourself, then run the command
  again.
- Codex is registered when its settings folder exists (`~/.codex`, or `CODEX_HOME`): one
  `gil` entry is added to `config.toml`; everything else in that file is kept, and a backup is
  written beside it before the change.
- Claude Desktop is reported as `NOT_INSTALLED`: GIL does not look for a desktop application that
  is not available on this platform.
- Any other MCP Host that runs a local stdio server can use GIL by registering it yourself:
  command `~/.local/share/gil/bin/gil` (write the full path), arguments `mcp --serve`.
  `gil install` does not edit other Hosts' settings.

`gil doctor` prints the state of the installation. Restart a registered Host before use.

## Starting a project

In a folder with no GIL record, the Agent asks which kind of new project to start. A project is
one kind or the other and is not converted later.

- **Tracked scope** — you approve, in the GIL Monitor, the exact `.gilignore` that decides which
  files GIL tracks and archives, and you can change that scope later with the same approval.
  Nothing is archived before the approval. Reports cannot hold image attachments, and restoring
  or revisiting work files is refused in such a project.
- **Image reports** — reports can hold PNG visuals. The project is created at once and observes
  the whole folder. It needs a local filesystem GIL has listed (ext4, XFS, Btrfs, tmpfs,
  overlayfs) and refuses elsewhere; it has no tracked-scope approval.

Both kinds keep Chains, approved plans, requirement-linked Experiments and original record reads.
Existing projects keep the kind and storage they were started with; installing or updating GIL
never converts a project.

The Monitor is shown by Hosts that support MCP Apps. Whether a given Host displays it is a
property of that Host; the tool answers do not claim that a screen is visible.

## Uninstall

```sh
~/.local/share/gil/bin/gil uninstall
```

removes the `gil` entry from the Host settings it registered. To remove the executable as well:

```sh
~/.local/share/gil/bin/gil uninstall --remove-binary
```

Only the executable and `install.json` recorded by the install are removed, and then the two
folders the install made when they are empty. A file at that path that the install did not record,
or that was changed afterwards, is kept and reported, and so is anything else you put in that folder. Projects, their `.gil` records and unrelated settings are never
touched by install or uninstall.

Third-party notices are in `THIRD-PARTY-NOTICES.txt`, `THIRD-PARTY-NOTICES.json` and
`RUST-STDLIB-NOTICES.html`; GIL's own license is `LICENSE`.
