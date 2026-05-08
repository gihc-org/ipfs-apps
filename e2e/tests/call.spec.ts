import { test, expect, Browser, Page } from '@playwright/test';
import { register, login, mockConfig, uniqueUser, API_URL } from './helpers';

async function newSession(browser: Browser, username: string, password: string) {
  const ctx = await browser.newContext({ permissions: ['microphone'] });
  const page = await ctx.newPage();
  await register(page, username, password);
  await login(page, username, password);
  // rooms.html stores userId asynchronously after /auth/me — wait for it
  await page.waitForFunction(() => !!localStorage.getItem('userId'), { timeout: 5000 });
  const token  = await page.evaluate(() => localStorage.getItem('token') ?? '');
  const userId = await page.evaluate(() => localStorage.getItem('userId') ?? '');
  return { page, token, userId };
}

async function openDm(page: Page, roomId: string, peerName: string, peerId: string) {
  await mockConfig(page);
  const params = new URLSearchParams({ room_id: roomId, room_name: peerName, is_dm: 'true', peer_id: peerId });
  await page.goto(`/chat.html?${params}`);
  // Wait for WS open — chat.html sets data-ws-connected on body in ws.onopen
  await page.waitForSelector('body[data-ws-connected]', { timeout: 10000 });
}

test('ring op → accepter → læg på', async ({ browser }) => {
  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  // Opret DM via API
  const dmRes = await alice.page.request.post(`${API_URL}/dms`, {
    headers: { Authorization: `Bearer ${alice.token}` },
    data: { user_id: bob.userId },
  });
  const dmBody = await dmRes.text();
  if (!dmRes.ok()) throw new Error(`DM POST failed ${dmRes.status()}: ${dmBody}`);
  const { room_id: roomId } = JSON.parse(dmBody);

  const aliceName = await alice.page.evaluate(() => localStorage.getItem('username') ?? '');
  const bobName   = await bob.page.evaluate(()   => localStorage.getItem('username') ?? '');

  await openDm(alice.page, roomId, bobName,   bob.userId);
  await openDm(bob.page,   roomId, aliceName, alice.userId);

  // Alice ringer op
  await alice.page.waitForSelector('#callBtn', { state: 'visible' });
  await alice.page.click('#callBtn');

  // Bob ser indgående opkald
  await bob.page.waitForSelector('#incomingCall.visible', { timeout: 10000 });

  // Bob accepterer
  await bob.page.click('button:has-text("Accepter")');

  // Begge ser aktivt opkald
  await alice.page.waitForSelector('#activeCall.visible', { timeout: 15000 });
  await bob.page.waitForSelector('#activeCall.visible',   { timeout: 5000  });

  // Alice lægger på
  await alice.page.click('button:has-text("Læg på")');

  // Begge ser opkaldet slut
  await expect(alice.page.locator('#activeCall')).not.toHaveClass(/visible/, { timeout: 5000 });
  await expect(bob.page.locator('#activeCall')).not.toHaveClass(/visible/,   { timeout: 5000 });
});

test('ring op → afvis', async ({ browser }) => {
  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  const dmRes = await alice.page.request.post(`${API_URL}/dms`, {
    headers: { Authorization: `Bearer ${alice.token}` },
    data: { user_id: bob.userId },
  });
  const dmBody = await dmRes.text();
  if (!dmRes.ok()) throw new Error(`DM POST failed ${dmRes.status()}: ${dmBody}`);
  const { room_id: roomId } = JSON.parse(dmBody);

  const aliceName = await alice.page.evaluate(() => localStorage.getItem('username') ?? '');
  const bobName   = await bob.page.evaluate(()   => localStorage.getItem('username') ?? '');

  await openDm(alice.page, roomId, bobName,   bob.userId);
  await openDm(bob.page,   roomId, aliceName, alice.userId);

  await alice.page.waitForSelector('#callBtn', { state: 'visible' });
  await alice.page.click('#callBtn');

  await bob.page.waitForSelector('#incomingCall.visible', { timeout: 10000 });
  await bob.page.click('button:has-text("Afvis")');

  // Alice ser callBtn vende tilbage til "Ring op"
  await expect(alice.page.locator('#callBtn')).toHaveText('Ring op', { timeout: 5000 });
  await expect(alice.page.locator('#incomingCall')).not.toHaveClass(/visible/);
});

test('30-sekunders timeout hvis ingen svarer', async ({ browser }) => {
  test.setTimeout(45000);

  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  const dmRes = await alice.page.request.post(`${API_URL}/dms`, {
    headers: { Authorization: `Bearer ${alice.token}` },
    data: { user_id: bob.userId },
  });
  const dmBody = await dmRes.text();
  if (!dmRes.ok()) throw new Error(`DM POST failed ${dmRes.status()}: ${dmBody}`);
  const { room_id: roomId } = JSON.parse(dmBody);

  const aliceName = await alice.page.evaluate(() => localStorage.getItem('username') ?? '');
  const bobName   = await bob.page.evaluate(()   => localStorage.getItem('username') ?? '');

  await openDm(alice.page, roomId, bobName,   bob.userId);
  await openDm(bob.page,   roomId, aliceName, alice.userId);

  await alice.page.waitForSelector('#callBtn', { state: 'visible' });
  await alice.page.click('#callBtn');

  // Ingen svarer — Alice ser "Intet svar" efter 30s
  await alice.page.waitForSelector('.system-msg:has-text("Intet svar")', { timeout: 35000 });
  await expect(alice.page.locator('#callBtn')).toHaveText('Ring op');
});
