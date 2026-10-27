import assert from 'node:assert/strict';
import test from 'node:test';
import {readFile} from 'node:fs/promises';

const here = new URL('../../', import.meta.url);
const read = path => readFile(new URL(path, here), 'utf8');

test('Windows product bundle is a current-user NSIS installer with signed updater artifacts', async () => {
  // Do not name this tauri.windows.conf.json: Tauri automatically merges that platform file into
  // every Windows cargo build, where product-only bootstrap resources do not exist.
  const config = JSON.parse(await read('companion/tauri.product-windows.conf.json'));
  assert.deepEqual(config.bundle.targets, ['nsis']);
  assert.equal(config.bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(config.bundle.createUpdaterArtifacts, true);
  assert.equal(config.plugins.updater.requireSignedVersion, true);
  assert.equal(config.plugins.updater.windows.installMode, 'basicUi');
  assert.equal(config.bundle.resources['../target/release/gil-launcher.exe'], 'bootstrap/gil-launcher.exe');
  assert.equal(config.bundle.resources['../target/windows-bootstrap/current.json'], 'bootstrap/current.json');
  assert.equal(config.bundle.windows.nsis.installerHooks, 'windows/runtime-hooks.nsh');
  await assert.rejects(read('companion/tauri.windows.conf.json'), /ENOENT/);
});

test('owner updater acceptance shows a native installer and still requires signed payloads', async () => {
  const config = JSON.parse(await read('companion/tauri.unsigned-updater-test-windows.conf.json'));
  assert.equal(config.productName, 'GIL Companion (Unsigned Test)');
  assert.equal(config.identifier, 'dev.ariadne.gil.companion.unsigned-test');
  assert.equal(config.bundle.createUpdaterArtifacts, true);
  assert.equal(config.plugins.updater.requireSignedVersion, true);
  assert.equal(config.plugins.updater.windows.installMode, 'basicUi');

  const script = await read('companion/build-windows-unsigned-updater-test.ps1');
  assert.match(script, /ValidatePattern\('\^\[0-9\]\+\\\.\[0-9\]\+\\\.\[0-9\]\+-owner/);
  assert.match(script, /TAURI_SIGNING_PRIVATE_KEY/);
  assert.match(script, /TAURI_SIGNING_PRIVATE_KEY_PASSWORD/);
  assert.match(script, /GIL_UPDATER_PUBLIC_KEY/);
  assert.match(script, /GIL_UPDATER_ENDPOINT/);
  assert.match(script, /tauri\.unsigned-updater-test-windows\.conf\.json/);
  assert.match(script, /version = \$Version; sha256 = \$Digest/);
  assert.doesNotMatch(script, /certificateThumbprint|timestampUrl|GIL_WINDOWS_CERTIFICATE_PFX/);
});

test('owner acceptance has a separate unsigned installer identity and cannot emit updater artifacts', async () => {
  const config = JSON.parse(await read('companion/tauri.unsigned-test-windows.conf.json'));
  assert.equal(config.productName, 'GIL Companion (Unsigned Test)');
  assert.equal(config.identifier, 'dev.ariadne.gil.companion.unsigned-test');
  assert.deepEqual(config.bundle.targets, ['nsis']);
  assert.equal(config.bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(config.bundle.createUpdaterArtifacts, false);
  assert.equal(config.bundle.windows.nsis.installerHooks, 'windows/runtime-hooks.nsh');
  assert.deepEqual(config.plugins.updater, {pubkey: ''});

  const script = await read('companion/build-windows-unsigned-test.ps1');
  assert.match(script, /UNSIGNED INTERNAL TEST ONLY/);
  assert.match(script, /Remove-Item "Env:\$Name"/);
  assert.match(script, /tauri\.unsigned-test-windows\.conf\.json/);
  assert.match(script, /\$RuntimeVersion = "\$CoreVersion-owner\.\$\(\$Digest\.Substring\(0, 12\)\)"/);
  assert.match(script, /Add-Member -NotePropertyName version -NotePropertyValue \$RuntimeVersion -Force/);
  assert.match(script, /ConvertTo-Json -Depth 10 -Compress/);
  assert.match(script, /cargo tauri build --bundles nsis --config \$UnsignedConfigJson/);
  assert.match(script, /version = \$RuntimeVersion; sha256 = \$Digest/);
  assert.doesNotMatch(script, /certificateThumbprint|timestampUrl/);
});

test('Companion keeps Codex connection visible and discovers the Desktop CLI without PATH', async () => {
  const clients = await read('companion/src/clients.rs');
  assert.match(clients, /cli_found: bool/);
  assert.match(clients, /join\("OpenAI"\).*join\("Codex"\).*join\("bin"\)/s);
  assert.match(clients, /starts_with\("\.staging-"\)/);
  assert.match(clients, /CODEX_CLI_PATH/);
  assert.match(clients, /starts_with\("codex-cli "\)/);

  const ui = await read('ui/companion.js');
  assert.match(ui, /function showCodexUnavailable/);
  assert.match(ui, /connectCodexButton\.hidden = false/);
  assert.match(ui, /Codex 연결 다시 시도/);
  assert.match(ui, /capability\.cli_found === false/);
  const startup = ui.slice(ui.indexOf('async function start()'));
  assert.doesNotMatch(startup, /catch \(error\) \{\s*connectCodexButton\.hidden = true;/);
});

test('installer stages immutable Core bytes and switches a recoverable receipt', async () => {
  const hook = await read('companion/windows/runtime-hooks.nsh');
  assert.match(hook, /runtime\\versions\\\$\{VERSION\}\\gil\.exe/);
  assert.match(hook, /already installed; preserving immutable version bytes/);
  assert.doesNotMatch(hook, /Abort "GIL Core .*already installed/);
  assert.match(hook, /IfFileExists .*gil-launcher\.exe.*launcher_ready/);
  assert.match(hook, /current\.next/);
  assert.match(hook, /current\.previous/);
  assert.match(hook, /previous version was restored/);
  assert.match(hook, /NSIS_HOOK_PREUNINSTALL/);
  assert.match(hook, /runtime and Project records are preserved/);
  const uninstallHook = hook.slice(hook.indexOf('NSIS_HOOK_PREUNINSTALL'));
  assert.doesNotMatch(uninstallHook, /RMDir \/r|Delete .*runtime/);
});

test('release build refuses missing update trust material', async () => {
  const script = await read('companion/build-windows.ps1');
  for (const name of [
    'GIL_UPDATER_PUBLIC_KEY',
    'GIL_UPDATER_ENDPOINT',
    'TAURI_SIGNING_PRIVATE_KEY',
    'GIL_WINDOWS_CERTIFICATE_THUMBPRINT',
    'GIL_WINDOWS_TIMESTAMP_URL',
  ]) assert.match(script, new RegExp(name));
  assert.match(script, /미서명 제품 설치본은 만들지 않습니다/);
  assert.match(script, /--bin gil --bin gil-launcher --features product-safety/);
  assert.match(script, /Get-FileHash -LiteralPath \$Core -Algorithm SHA256/);
  assert.match(script, /architecture = 'x86_64'/);
  assert.match(script, /cohort = 'gil-mcp-v1'/);
  assert.match(script, /current\.json/);
  assert.match(script, /certificateThumbprint = \$Thumbprint/);
  assert.match(script, /digestAlgorithm = 'sha256'/);
  assert.match(script, /Tauri CLI 2\.10\.1/);
  assert.match(script, /UpdaterEndpoint\.Scheme -ne 'https'/);
  assert.match(script, /--config tauri\.product-windows\.conf\.json/);
});

test('the updater has no automatic check and install requires a second user decision', async () => {
  const ui = await read('ui/companion.js');
  const check = ui.indexOf('async function checkForUpdate');
  const start = ui.indexOf('async function start');
  assert.ok(check >= 0 && start > check);
  assert.ok(!ui.slice(start).includes('window.GIL_HOST.checkUpdate()'));
  const body = ui.slice(check, start);
  assert.ok(body.indexOf('window.confirm(') < body.indexOf('window.GIL_HOST.installUpdate('));

  const rust = await read('companion/src/updates.rs');
  assert.match(rust, /download_and_install/);
  assert.match(rust, /\.pubkey\(key\)/);
  assert.match(rust, /expected_version/);
  assert.match(rust, /app\.package_info\(\)\.version\.to_string\(\)/);
});

test('the update UI keeps Codex open and explains the connection boundary', async () => {
  const ui = await read('ui/companion.js');
  const check = ui.indexOf('async function checkForUpdate');
  const connect = ui.indexOf('async function connectCodex', check);
  const body = ui.slice(check, connect);
  assert.match(body, /Codex는 닫지 않아도 됩니다/);
  assert.match(body, /다운로드가 끝나면 Windows 설치창이 열립니다/);
  assert.match(body, /완료되면 설치창이 열립니다/);
  assert.match(body, /기존 GIL 대화는 현재 Core로 계속/);
  assert.match(body, /새 대화부터 새 Core/);
  assert.match(body, /Companion만 닫힐 수/);
});

test('release GUI does not open a Windows console', async () => {
  assert.match(await read('companion/src/main.rs'), /windows_subsystem = "windows"/);
});

test('Codex registration is explicit, uses the official CLI shape, and does not overwrite', async () => {
  const clients = await read('companion/src/clients.rs');
  assert.match(clients, /\["mcp", "add", "gil", "--"\]/);
  assert.match(clients, /gil-launcher\.exe/);
  assert.match(clients, /기존 gil 등록은 자동으로 덮어쓰지 않는다/);
  assert.match(clients, /\["mcp", "list", "--json"\]/);
  assert.match(clients, /\["mcp", "remove", "gil"\]/);
  assert.match(clients, /Registration::Conflict/);
  assert.match(clients, /\["plugin", "list", "--json"\]/);
  assert.match(clients, /\["plugin", "remove", &plugin, "--json"\]/);
  assert.match(clients, /gil-companion-prototype/);

  const ui = await read('ui/companion.js');
  const connect = ui.indexOf('async function connectCodex');
  assert.ok(connect >= 0);
  const body = ui.slice(connect, ui.indexOf('async function start', connect));
  assert.ok(body.indexOf('window.confirm(') < body.indexOf('window.GIL_HOST.connectCodex()'));
  const disconnect = ui.indexOf('async function disconnectCodex');
  assert.ok(disconnect > connect);
  const disconnectBody = ui.slice(disconnect, ui.indexOf('async function refreshCodexConnection', disconnect));
  assert.ok(disconnectBody.indexOf('window.confirm(') < disconnectBody.indexOf('window.GIL_HOST.disconnectCodex()'));
  const repair = ui.indexOf('async function repairCodexConnection');
  assert.ok(repair > disconnect);
  const repairBody = ui.slice(repair, ui.indexOf('async function refreshCodexConnection', repair));
  assert.ok(repairBody.indexOf('window.confirm(') < repairBody.indexOf('window.GIL_HOST.repairCodexConnection()'));
});

test('signed product CI is manual, secret-gated, installed, and exercised before retention', async () => {
  const workflow = await read('.github/workflows/windows-product.yml');
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /if: github\.event_name == 'workflow_dispatch' && inputs\.build_signed_candidate/);
  assert.match(workflow, /environment: windows-product-signing/);
  for (const secret of [
    'GIL_UPDATER_PRIVATE_KEY',
    'GIL_WINDOWS_CERTIFICATE_PFX_BASE64',
    'GIL_WINDOWS_CERTIFICATE_PASSWORD',
  ]) assert.match(workflow, new RegExp(`secrets\\.${secret}`));
  assert.match(workflow, /cargo install tauri-cli --version 2\.10\.1 --locked/);
  assert.match(workflow, /1\.3\.6\.1\.5\.5\.7\.3\.3/);
  assert.match(workflow, /product-e2e\.ps1/);
  assert.match(workflow, /DisplayName 'GIL Companion \(Unsigned Test\)' -SeedLegacyRuntime/);
  assert.match(workflow, /Owner candidate is missing its content-specific version/);
  assert.ok(workflow.indexOf('product-e2e.ps1') < workflow.indexOf('actions\/upload-artifact'));

  const acceptance = await read('distribution/windows/product-e2e.ps1');
  assert.match(acceptance, /\[switch\]\$SeedLegacyRuntime/);
  assert.match(acceptance, /Owner candidate reused the legacy runtime directory/);
  assert.match(acceptance, /Installation changed or removed the preserved legacy runtime/);
  assert.match(acceptance, /Start-Process -FilePath \$Installer -ArgumentList '\/S'/);
  assert.match(acceptance, /product-launcher-smoke\.mjs/);
  assert.match(acceptance, /product-launcher-smoke\.mjs \$Launcher \$PreviousCore \$Installer \$ReceiptPath \$Receipt\.version/);
  assert.match(acceptance, /version_switch_while_mcp_connected = \$true/);
  assert.match(acceptance, /old_connection_survived_update = \$true/);
  assert.match(acceptance, /new_core_selected_for_next_connection = \$true/);
  assert.match(acceptance, /mcp add gil -- \$Launcher mcp --serve/);
  assert.match(acceptance, /mcp remove gil/);
  assert.match(acceptance, /Get-AuthenticodeSignature/);
  assert.match(acceptance, /MainWindowHandle/);
  assert.match(acceptance, /visible_companion_window = \$true/);
  assert.match(acceptance, /RedirectStandardError \$GuiStderr/);
  assert.match(acceptance, /Start-Process -FilePath \$Uninstaller -ArgumentList '\/S'/);
  assert.match(acceptance, /Uninstall removed the preserved runtime/);

  const launcherSmoke = await read('distribution/windows/product-launcher-smoke.mjs');
  assert.match(launcherSmoke, /spawnSync\(installer, \['\/S'\]/);
  assert.match(launcherSmoke, /afterUpdate = await request\('tools\/list'/);
  assert.match(launcherSmoke, /selected\.version, expectedVersion/);
  assert.match(launcherSmoke, /version_switch_while_mcp_connected: 'passed'/);
});

test('unsigned owner acceptance CI is manual, isolated, install-tested and never published', async () => {
  const workflow = await read('.github/workflows/windows-product.yml');
  assert.match(workflow, /build_unsigned_test_candidate:/);
  assert.match(workflow, /if: github\.event_name == 'workflow_dispatch' && inputs\.build_unsigned_test_candidate/);
  assert.match(workflow, /UNSIGNED owner acceptance candidate/);
  assert.match(workflow, /build-windows-unsigned-test\.ps1/);
  assert.match(workflow, /-ExpectedSignature NotSigned/);
  assert.match(workflow, /GIL Companion \(Unsigned Test\)/);
  assert.match(workflow, /retention-days: 3/);
  assert.doesNotMatch(workflow.slice(workflow.indexOf('unsigned-test-candidate:'), workflow.indexOf('unsigned-updater-candidate:')), /environment: windows-product-signing|secrets\./);

  const acceptance = await read('distribution/windows/product-e2e.ps1');
  assert.match(acceptance, /ValidateSet\('Valid', 'NotSigned'\)/);
  assert.match(acceptance, /expected \$\{ExpectedSignature\}:/);
  assert.match(acceptance, /Where-Object DisplayName -eq \$DisplayName/);
  assert.match(acceptance, /if \(\$Signature\.SignerCertificate\)/);
});

test('owner updater CI needs a monotonic version and updater secrets but no paid certificate', async () => {
  const workflow = await read('.github/workflows/windows-product.yml');
  assert.match(workflow, /build_unsigned_updater_candidate:/);
  assert.match(workflow, /owner_update_version:/);
  const start = workflow.indexOf('  unsigned-updater-candidate:');
  const end = workflow.indexOf('  signed-candidate:', start);
  assert.ok(start >= 0 && end > start);
  const job = workflow.slice(start, end);
  assert.match(job, /inputs\.build_unsigned_updater_candidate/);
  assert.match(job, /build-windows-unsigned-updater-test\.ps1 -Version/);
  assert.match(job, /secrets\.GIL_UPDATER_PRIVATE_KEY/);
  assert.match(job, /secrets\.GIL_UPDATER_PRIVATE_KEY_PASSWORD/);
  assert.match(job, /Get-AuthenticodeSignature/);
  assert.match(job, /NotSigned/);
  assert.match(job, /Get-ChildItem target\\release\\bundle\\nsis -File -Filter '\*\.sig'/);
  assert.doesNotMatch(job, /GIL_WINDOWS_CERTIFICATE|Import-PfxCertificate|timestampUrl/);
});

test('the owner update manifest is produced by a validated static-manifest writer', async () => {
  const writer = await read('distribution/windows/write-owner-update-manifest.mjs');
  assert.match(writer, /windows-x86_64/);
  assert.match(writer, /signature/);
  assert.match(writer, /url\.protocol !== 'https:'/);
  assert.match(writer, /flag: 'wx'/);
});

test('local Windows acceptance separates contract checks from signed installation', async () => {
  const runner = await read('distribution/windows/run-local-acceptance.ps1');
  assert.match(runner, /ValidateSet\('Contract', 'Signed'\)/);
  assert.match(runner, /if \(\$Mode -eq 'Contract'\)/);
  assert.match(runner, /if \(-not \$AllowInstallTest\)/);
  assert.match(runner, /companion\\build-windows\.ps1/);
  assert.match(runner, /distribution\\windows\\product-e2e\.ps1/);
});
