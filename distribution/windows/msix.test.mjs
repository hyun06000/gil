// Contract of the Microsoft Store package: the manifest template, its assets and the packaging
// script. Pure Node; the real pack/sign/sideload runs in windows-release.yml.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile, readdir} from 'node:fs/promises';

const here = new URL('./msix/', import.meta.url);
const read = async name => readFile(new URL(name, here), 'utf8');

test('the manifest template declares exactly the capabilities the single exe needs', async () => {
  const xml = await read('AppxManifest.xml');
  for (const placeholder of ['{{IDENTITY_NAME}}', '{{PUBLISHER}}', '{{PUBLISHER_DISPLAY_NAME}}', '{{VERSION}}']) {
    assert.equal(xml.split(placeholder).length, 2, `${placeholder} appears once`);
  }
  assert.match(xml, /<rescap:Capability Name="runFullTrust" \/>/);
  assert.match(xml, /<rescap:Capability Name="unvirtualizedResources" \/>/);
  assert.match(xml, /<desktop6:FileSystemWriteVirtualization>disabled<\/desktop6:FileSystemWriteVirtualization>/);
  assert.match(xml, /<uap5:ExecutionAlias Alias="gil\.exe" \/>/);
  assert.match(xml, /Executable="gil\.exe"\s+EntryPoint="Windows\.FullTrustApplication"\s+desktop4:Subsystem="console"/);
  assert.match(xml, /ProcessorArchitecture="x64"/);
  // Store validation requires the package display name to be a reserved name of the app.
  assert.match(xml, /<DisplayName>gil-companion<\/DisplayName>/);
  assert.match(xml, /VisualElements\s+DisplayName="gil-companion"/);
  // Nothing that would reach outside the user's profile or run at install time.
  assert.doesNotMatch(xml, /internetClient|broadFileSystemAccess|startupTask|desktop:Extension|windows\.service|allowElevation/);
  assert.doesNotMatch(xml, /http:\/\/(?!schemas\.microsoft\.com)/);
  const logos = [...xml.matchAll(/Assets\\([A-Za-z0-9x]+\.png)/g)].map(m => m[1]);
  const present = (await readdir(new URL('Assets/', here))).filter(n => n.endsWith('.png'));
  for (const logo of new Set(logos)) assert.ok(present.includes(logo), `${logo} is referenced but missing`);
});

test('tile assets have the pixel sizes their names promise', async () => {
  const size = async name => {
    const bytes = await readFile(new URL(`Assets/${name}`, here));
    assert.equal(bytes.toString('latin1', 1, 4), 'PNG', name);
    return [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
  };
  for (const [name, w, h] of [['Square44x44Logo.png', 44, 44], ['Square71x71Logo.png', 71, 71], ['Square150x150Logo.png', 150, 150],
    ['Square310x310Logo.png', 310, 310], ['Wide310x150Logo.png', 310, 150], ['StoreLogo.png', 50, 50]]) {
    assert.deepEqual(await size(name), [w, h], name);
  }
});

test('the packaging script fills every placeholder, packs with makeappx and signs only on request', async () => {
  const script = await read('build-msix.ps1');
  for (const placeholder of ['{{IDENTITY_NAME}}', '{{PUBLISHER}}', '{{PUBLISHER_DISPLAY_NAME}}', '{{VERSION}}']) {
    assert.ok(script.includes(`'${placeholder}'`), placeholder);
  }
  assert.match(script, /if \(\$manifest -match '\\\{\\\{\[A-Z_\]\+\\\}\\\}'\) \{ throw/);
  assert.match(script, /makeappx pack \/o \/d \$Layout \/p \$Out/);
  assert.match(script, /if \(\$SignWithPfx\) \{[\s\S]*signtool sign \/fd SHA256/);
  assert.match(script, /"\$Version\.0"/, 'Store packages use a four-part version ending in .0');
  assert.doesNotMatch(script, /Add-AppxPackage|Install-|Invoke-WebRequest|irm /);
});
