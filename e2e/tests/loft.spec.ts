import { expect, test, Browser, Page } from '@playwright/test';
import { mockConfig, uniqueName } from './helpers';

async function newParticipant(browser: Browser) {
  const ctx = await browser.newContext({ permissions: ['microphone', 'camera'] });
  await ctx.addInitScript(() => sessionStorage.setItem('loft.noAutoMic', '1'));
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

async function enableMic(page: Page) {
  await page.click('#micBtn');
  await expect(page.locator('#micBtn')).toHaveText('Sluk mikrofon');
}

test('to deltagere forbinder — lyd når frem', async ({ browser }) => {
  const alice = await newParticipant(browser);
  const bob = await newParticipant(browser);

  const id = await createLoft(alice.page, uniqueName('alice'));
  await joinLoft(bob.page, uniqueName('bob'), id);

  await expectConnected(alice.page);
  await expectConnected(bob.page);

  // Hver side ser den anden som ét kort med én remote audio-stream.
  await expect(alice.page.locator('#grid .participant-card')).toHaveCount(1);
  await expect(bob.page.locator('#grid .participant-card')).toHaveCount(1);

  // Mikrofoner startes sekventielt (ikke samtidigt) for at undgå offer-glare
  // mellem to nye deltagere — samme sti som skærmdelingstesten.
  await enableMic(alice.page);
  await expect(bob.page.locator('#grid audio')).toHaveCount(1, { timeout: 10000 });
  await enableMic(bob.page);
  await expect(alice.page.locator('#grid audio')).toHaveCount(1, { timeout: 10000 });
});

test('forlad → leave → rejoin', async ({ browser }) => {
  const alice = await newParticipant(browser);
  const bob = await newParticipant(browser);

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
  const carol = await newParticipant(browser);
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
  const bob = await newParticipant(browser);

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
