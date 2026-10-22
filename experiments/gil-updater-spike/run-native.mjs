import {mkdtemp} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
const cwd = dirname(fileURLToPath(import.meta.url));
if (process.env.GIL_REQUIRE_WINDOWS === '1' && process.platform !== 'win32') throw Error('Windows execution required');
if (process.env.GIL_EXPECT_ARCH && process.arch !== process.env.GIL_EXPECT_ARCH) throw Error('Native Node architecture mismatch');
const directory = await mkdtemp(join(tmpdir(), 'gil-native-fixtures-'));
const rustInfo = spawnSync('rustc', ['-vV'], {encoding:'utf8'});
if (rustInfo.status !== 0) throw Error(rustInfo.stderr || 'rustc unavailable');
const expectedTarget = {x64:'x86_64-pc-windows-msvc', arm64:'aarch64-pc-windows-msvc'}[process.arch];
if (process.platform === 'win32' && (!expectedTarget || !rustInfo.stdout.includes(`host: ${expectedTarget}`))) {
  throw Error('Native Windows MSVC rustc host required');
}
console.log(JSON.stringify({platform:process.platform, arch:process.arch, node:process.version, directory}));
console.log(rustInfo.stdout);
for (const [name,version,broken] of [['v1','v1','0'],['v2','v2','0'],['broken','v2','1']]) {
  const result = spawnSync('rustc', [join(cwd,'native-core.rs'), '-o',join(directory,`${name}.exe`)], {
    stdio:'inherit',env:{...process.env,GIL_SPIKE_VERSION:version,GIL_SPIKE_BROKEN:broken}
  });
  if (result.status !== 0) throw Error(`fixture compile failed: ${name}`);
}
const result = spawnSync(process.execPath,['--test',join(cwd,'native.test.mjs')],{
  stdio:'inherit',env:{...process.env,GIL_NATIVE_FIXTURES:directory},timeout:60000
});
process.exitCode = result.status ?? 1;
