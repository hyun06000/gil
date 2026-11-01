// **Project 자리의 글자** — `node --test`
//
// server.mjs 의 사전 검사는 Core 의 `resolve_root` 와 같은 말을 해야 한다: 받은 글자가 폴더면 그대로,
// 한글 정규화(NFC/NFD)만 다르면 있는 쪽을 쓰고 한 번 알리며, 호환 자모는 어느 폴더에도 맞출 수 없다.
import test from "node:test";
import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

process.env.GIL_COMPANION_NO_SERVE = "1";
const { usableDirectory } = await import("./server.mjs");

const NFC = "\u{D55C}\u{AE00}";
const NFD = "\u{1112}\u{1161}\u{11AB}\u{1100}\u{1173}\u{11AF}";

test("a directory is used as given, with no note", async () => {
  const base = await mkdtemp(join(tmpdir(), "gil-root-"));
  try {
    assert.deepEqual(await usableDirectory(base), { root: base, note: null });
  } finally { await rm(base, { recursive: true, force: true }); }
});

test("the other Hangul normal form reaches the same folder and is announced once", async () => {
  const base = await mkdtemp(join(tmpdir(), "gil-root-"));
  try {
    for (const [made, asked] of [[NFD, NFC], [NFC, NFD]]) {
      const root = join(base, `${made}-${made.length}`);
      await mkdir(root);
      const other = join(base, `${asked}-${made.length}`);
      const direct = await stat(other).then(() => true, () => false);
      const place = await usableDirectory(other);
      assert.equal(place.problem, undefined);
      if (direct) {
        assert.deepEqual(place, { root: other, note: null });
      } else {
        assert.equal(place.root, root);
        assert.match(place.note, /^project_root 글자를 맞춰 .* 를 사용했다/);
      }
    }
  } finally { await rm(base, { recursive: true, force: true }); }
});

test("a missing folder and compatibility jamo are refused with what to copy", async () => {
  const base = await mkdtemp(join(tmpdir(), "gil-root-"));
  try {
    const missing = await usableDirectory(join(base, NFC, "없는자리"));
    assert.match(missing.problem, /^Project 자리를 열 수 없다: 경로가 존재하지 않는다\./);
    assert.match(missing.problem, /한글 정규화/);
    assert.match(missing.problem, /글자 그대로 복사/);
    const apart = await usableDirectory(join(base, "\u{314E}\u{314F}\u{3134}"));
    assert.match(apart.problem, /^Project 자리를 열 수 없다: 경로에 호환 자모/);
    assert.equal((await usableDirectory("")).problem, "Project 자리가 비었다");
    assert.equal((await usableDirectory(undefined)).problem, "Project 자리가 비었다");
  } finally { await rm(base, { recursive: true, force: true }); }
});
