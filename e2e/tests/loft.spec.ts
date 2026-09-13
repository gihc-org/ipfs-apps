import { expect, test, Browser, Page } from '@playwright/test';
import { mockConfig, uniqueName } from './helpers';

async function newParticipant(browser: Browser, noAutoMic = false) {
  // Firefox understøtter ikke Playwrights permission-grant; der auto-grantes i
  // stedet via firefoxUserPrefs i playwright.config.ts.
  const permissions =
    browser.browserType().name() === 'firefox' ? {} : { permissions: ['microphone', 'camera'] };
  const ctx = await browser.newContext(permissions);
  if (noAutoMic) {
    await ctx.addInitScript(() => sessionStorage.setItem('loft.noAutoMic', '1'));
  }
  const page = await ctx.newPage();
  await mockConfig(page);
  return { ctx, page };
}

async function createLoft(page: Page, name: string): Promise<string> {
  await page.goto('/loft.html');
  await page.fill('#nameInput', name);
  await page.click('#createBtn');
  await expect(page.locator('#room')).toBeVisible({ timeout: 10000 });
  const id = new URL(page.url()).searchParams.get('id');
  expect(id).toBeTruthy();
  return id!;
}

async function joinLoft(page: Page, name: string, id: string) {
  await page.goto(`/loft.html?id=${id}`);
  await page.fill('#nameInput', name);
  await page.click('#joinBtn');
  await expect(page.locator('#room')).toBeVisible({ timeout: 10000 });
}

async function expectConnected(page: Page) {
  await page.waitForFunction(() => {
    const debug = (window as any).__loftDebug;
    if (!debug) return false;
    const { peers } = debug();
    return peers.length > 0 && peers.every((p: any) => p.connectionState === 'connected');
  }, undefined, { timeout: 15000 });
}

test('to deltagere forbinder — lyd når frem', async ({ browser }) => {
  const alice = await newParticipant(browser);
  const bob = await newParticipant(browser, true);

  const id = await createLoft(alice.page, uniqueName('alice'));
  await joinLoft(bob.page, uniqueName('bob'), id);

  await expectConnected(alice.page);
  await expectConnected(bob.page);

  // Hver side ser den anden som ét kort med én remote audio-stream.
  await expect(alice.page.locator('#grid .participant-card')).toHaveCount(1);
  await expect(bob.page.locator('#grid .participant-card')).toHaveCount(1);

  // Alice's auto-mikrofon etablerede forbindelsen — Bob modtager hendes lyd.
  await expect(bob.page.locator('#grid audio')).toHaveCount(1, { timeout: 10000 });

  // Bob starter sin mikrofon sekventielt (ingen offer-glare) — Alice hører ham.
  await bob.page.click('#micBtn');
  await expect(bob.page.locator('#micBtn')).toHaveText('Sluk mikrofon');
  await expect(alice.page.locator('#grid audio')).toHaveCount(1, { timeout: 10000 });
});

test('forlad → leave → rejoin', async ({ browser }) => {
  const alice = await newParticipant(browser);
  const bob = await newParticipant(browser, true);

  const id = await createLoft(alice.page, uniqueName('alice'));
  await joinLoft(bob.page, uniqueName('bob'), id);
  await expectConnected(alice.page);
  await expectConnected(bob.page);

  // Alice forlader — Bob mister kortet.
  await alice.page.click('#leaveBtn');
  await expect(alice.page.locator('#room')).toBeHidden();
  await expect(bob.page.locator('#grid .participant-card')).toHaveCount(0, { timeout: 10000 });

  // Alice joiner igen via det samme link (navn/loft-id står stadig i lobbyen).
  await alice.page.click('#joinBtn');
  await expect(alice.page.locator('#room')).toBeVisible({ timeout: 10000 });
  await expectConnected(alice.page);
  await expectConnected(bob.page);
  await expect(bob.page.locator('#grid .participant-card')).toHaveCount(1);
});

test('link åbnes fra frisk kontekst (ny deltager)', async ({ browser }) => {
  const alice = await newParticipant(browser);
  const id = await createLoft(alice.page, uniqueName('alice'));

  const origin = await alice.page.evaluate(() => location.origin);
  const carol = await newParticipant(browser, true);
  await carol.page.goto(`${origin}/loft.html?id=${id}`);
  await carol.page.fill('#nameInput', uniqueName('carol'));
  await carol.page.click('#joinBtn');
  await expect(carol.page.locator('#room')).toBeVisible({ timeout: 10000 });

  await expectConnected(alice.page);
  await expectConnected(carol.page);
  await expect(alice.page.locator('#grid .participant-card')).toHaveCount(1);
  await expect(carol.page.locator('#grid .participant-card')).toHaveCount(1);
});

test('skærmdeling når frem (fake stream)', async ({ browser }) => {
  const alice = await newParticipant(browser);
  const bob = await newParticipant(browser, true);

  const id = await createLoft(alice.page, uniqueName('alice'));
  await joinLoft(bob.page, uniqueName('bob'), id);
  await expectConnected(alice.page);
  await expectConnected(bob.page);

  // Skærm-capture kan ikke automatiseres pålideligt i headless Chromium —
  // derfor startes en canvas-stream via test-hooken (samme RTC-sti som
  // getDisplayMedia: addMedia → renegotiation → remote video-track).
  await alice.page.evaluate(() => (window as any).__loftStartFakeScreen());

  await bob.page.waitForFunction(
    () => document.querySelectorAll('#grid .participant-card video').length >= 1,
    undefined,
    { timeout: 15000 },
  );
  await alice.page.waitForFunction(() => {
    const d = (window as any).__loftDebug?.();
    return !!d && d.screenOn;
  }, undefined, { timeout: 5000 });
});

// Fund 2026-09-12: WebRTC-lyden gik ud af telefonens højttaler i stedet for
// Bluetooth-earpluggene. Vælgeren dukker kun op hvor browseren eksponerer
// audiooutput-enheder (desktop Firefox/Chromium — ikke Android).
test('lydudgang kan vælges og sættes på fjernlyden (setSinkId)', async ({ browser }) => {
  const alice = await newParticipant(browser);
  const bob = await newParticipant(browser, true);

  const id = await createLoft(alice.page, uniqueName('alice'));
  await joinLoft(bob.page, uniqueName('bob'), id);
  await expectConnected(alice.page);
  await expectConnected(bob.page);

  // Bob får først output-listen når mikrofonen (og dermed tilladelsen) er der.
  await bob.page.click('#micBtn');
  await expect(bob.page.locator('#grid audio')).toHaveCount(1, { timeout: 10000 });

  // Enhedslisten fyldes asynkront efter tilladelsen — vent kort på den, men
  // fejl ikke på runners uden lydenheder (der findes ingen audiooutput der).
  const debug = await bob.page.evaluate(async () => {
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      const d = (window as any).__loftDebug?.();
      if (d && (!d.sinkSupported || d.outputDevices.length > 0)) return d;
      await new Promise((r) => setTimeout(r, 100));
    }
    return (window as any).__loftDebug();
  });
  test.skip(!debug.sinkSupported, 'browseren har ikke HTMLMediaElement.setSinkId');
  test.skip(!debug.sinkSelectable, 'browseren eksponerer ingen vælgbare audiooutput-enheder');

  await expect(bob.page.locator('#sinkWrap')).toBeVisible({ timeout: 10000 });
  const choices: string[] = await bob.page.$$eval('#sinkSelect option', (els) =>
    els.map((e) => (e as HTMLOptionElement).value),
  );
  const deviceId = choices.find((v) => v);
  expect(deviceId).toBeTruthy();

  await bob.page.selectOption('#sinkSelect', deviceId!);
  await bob.page.waitForFunction(
    (want) => {
      const el = document.querySelector<HTMLAudioElement>('#grid audio');
      return !!el && el.sinkId === want;
    },
    deviceId,
    { timeout: 5000 },
  );
});
