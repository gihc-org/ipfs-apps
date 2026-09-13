import { expect, test } from '@playwright/test';

// Diagnostikværktøjet til lyd-routing (frontend/audio-debug.html) laves til
// telefon-test, men selve mekanikken — to RTCPeerConnections i samme side,
// remote track ind i et <audio>-element og setSinkId — kan verificeres her.
test('audio-debug: fjernlyd fra WebRTC i samme side og setSinkId-support', async ({ page }) => {
  await page.goto('/audio-debug.html');

  const support = await page.evaluate(() => (window as any).__audioDebug.support());
  expect(support.htmlMediaElementSetSinkId).toBe(true);
  await expect(page.locator('#envBody')).toContainText('setSinkId');
  await expect(page.locator('#deviceList')).not.toBeEmpty();

  await page.click('#toneBtn');
  await expect(page.locator('#toneAudio')).toHaveCount(1);

  await page.waitForFunction(() => {
    const report = (window as any).__audioDebug.report();
    return report.tone
      && report.tone.pcA === 'connected'
      && report.tone.pcB === 'connected'
      && report.tone.audioPaused === false;
  }, undefined, { timeout: 15000 });

  const report = await page.evaluate(() => (window as any).__audioDebug.report());
  expect(report.errors).toEqual([]);
  expect(report.tone.withVideo).toBe(false);
  expect(typeof report.tone.sinkId).toBe('string');

  // Stop-knappen river forbindelsen ned igen uden fejl.
  await page.click('#toneBtn');
  await expect(page.locator('#toneAudio')).toHaveCount(0);
  const after = await page.evaluate(() => (window as any).__audioDebug.report());
  expect(after.tone).toBeNull();
  expect(after.errors).toEqual([]);
});

test('audio-debug: videospor i loopback (kamera/skærm-scenariet)', async ({ page }) => {
  await page.goto('/audio-debug.html');
  await page.check('#includeVideo');
  await page.click('#toneBtn');

  await page.waitForFunction(() => {
    const report = (window as any).__audioDebug.report();
    return report.tone && report.tone.withVideo && report.tone.audioPaused === false;
  }, undefined, { timeout: 15000 });

  const report = await page.evaluate(() => (window as any).__audioDebug.report());
  expect(report.errors).toEqual([]);
});

// Siden skal også kunne bruges fra en almindelig http-origin (fx LAN-IP på en
// telefon), hvor navigator.mediaDevices ikke findes: tone-testen skal stadig
// kunne køre, for det er den der afgør hvor fjernlyden kommer ud.
test('audio-debug: degraderer pænt uden mediaDevices (http-origin)', async ({ page }) => {
  await page.addInitScript(() => {
    // mediaDevices er en accessor på Navigator.prototype — den skal fjernes der
    // for at efterligne en almindelig http-origin.
    Object.defineProperty(Navigator.prototype, 'mediaDevices', {
      configurable: true,
      get: () => undefined,
    });
  });
  await page.goto('/audio-debug.html');

  await expect(page.locator('#envBody')).toContainText('findes IKKE');
  await expect(page.locator('#deviceList')).toContainText('utilgængeligt');

  await page.click('#toneBtn');
  await page.waitForFunction(() => {
    const report = (window as any).__audioDebug.report();
    return report.tone && report.tone.audioPaused === false;
  }, undefined, { timeout: 15000 });

  const report = await page.evaluate(() => (window as any).__audioDebug.report());
  expect(report.errors).toEqual([]);
});
