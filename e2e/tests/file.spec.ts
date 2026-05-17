import { test, expect, Browser } from '@playwright/test';
import { register, login, mockConfig, uniqueUser, deleteUser, API_URL } from './helpers';

const TEST_FILE = {
  name: 'testfil.txt',
  mimeType: 'text/plain',
  buffer: Buffer.from('hej verden — denne fil er en testfil'),
};

async function newSession(browser: Browser, username: string, password: string) {
  const ctx = await browser.newContext();
  const page = await ctx.newPage();
  await register(page, username, password);
  await login(page, username, password);
  await page.waitForFunction(() => !!localStorage.getItem('userId'), { timeout: 5000 });
  const token  = await page.evaluate(() => localStorage.getItem('token') ?? '');
  const userId = await page.evaluate(() => localStorage.getItem('userId') ?? '');
  return { page, token, userId };
}

async function openDm(page: Parameters<typeof mockConfig>[0], roomId: string, peerName: string, peerId: string) {
  await mockConfig(page);
  const params = new URLSearchParams({ room_id: roomId, room_name: peerName, is_dm: 'true', peer_id: peerId });
  await page.goto(`/chat.html?${params}`);
  await page.waitForSelector('body[data-ws-connected]', { timeout: 10000 });
}

async function createDm(page: Parameters<typeof mockConfig>[0], token: string, targetUserId: string) {
  const res = await page.request.post(`${API_URL}/dms`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { user_id: targetUserId },
  });
  if (!res.ok()) throw new Error(`DM POST fejlede ${res.status()}: ${await res.text()}`);
  const { room_id } = await res.json();
  return room_id as string;
}

test('sender fil via backend når modtager ikke er i rummet', async ({ browser }) => {
  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  const roomId  = await createDm(alice.page, alice.token, bob.userId);
  const bobName = await bob.page.evaluate(() => localStorage.getItem('username') ?? '');

  // Alice åbner DM-rummet — Bob gør ikke (peerOnline = false)
  await openDm(alice.page, roomId, bobName, bob.userId);

  await alice.page.setInputFiles('#fileInput', TEST_FILE);

  // Filen uploades til backend → besked med downloadknap vises
  await alice.page.waitForSelector('.file-link', { timeout: 15000 });
  await expect(alice.page.locator('.file-link')).toContainText('testfil.txt');

  // Bob åbner rummet og ser downloadknappen i historikken
  const aliceName = await alice.page.evaluate(() => localStorage.getItem('username') ?? '');
  await openDm(bob.page, roomId, aliceName, alice.userId);
  await bob.page.waitForSelector('.file-link', { timeout: 5000 });
  await expect(bob.page.locator('.file-link')).toContainText('testfil.txt');

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
});

test('sender fil P2P når begge er i DM-rummet', async ({ browser }) => {
  test.setTimeout(45000);

  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  const roomId    = await createDm(alice.page, alice.token, bob.userId);
  const aliceName = await alice.page.evaluate(() => localStorage.getItem('username') ?? '');
  const bobName   = await bob.page.evaluate(() => localStorage.getItem('username') ?? '');

  // Begge åbner DM-rummet
  await openDm(alice.page, roomId, bobName,   bob.userId);
  await openDm(bob.page,   roomId, aliceName, alice.userId);

  // Vent til Alice har set Bobs join-besked (peerOnline = true)
  await alice.page.waitForSelector('.system-msg', { timeout: 8000 });

  // Alice sender filen — forsøger P2P (DataChannel)
  await alice.page.setInputFiles('#fileInput', TEST_FILE);

  // Bob ser enten progress-element (P2P igangsat) eller downloadknap (P2P fuldført)
  // Accept begge: 10s fallback til backend er mulig i CI uden TURN
  await bob.page.waitForSelector('.file-transfer, .file-link', { timeout: 20000 });

  // Alice ser progress eller sendt-bekræftelse
  await alice.page.waitForSelector('.file-transfer', { timeout: 5000 });

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
});

test('falder tilbage til backend hvis P2P-forbindelsen ikke åbner inden 10s', async ({ browser }) => {
  // Simulerer: Alice tror Bob er online (presence-mock), men Bob er ikke i DM-rummet.
  // Ingen DataChannel-åbning → 10s fallback → backend upload.
  test.setTimeout(30000);

  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  const roomId  = await createDm(alice.page, alice.token, bob.userId);
  const bobName = await bob.page.evaluate(() => localStorage.getItem('username') ?? '');

  // Intercept presence-kald så Alice tror Bob er online (men Bob er ikke i rummet)
  await alice.page.route(`${API_URL}/presence`, route =>
    route.fulfill({ contentType: 'application/json', body: JSON.stringify({ online: [bob.userId] }) })
  );

  // Alice åbner DM-rummet — checkPeerPresence() sætter peerOnline = true via mock
  await openDm(alice.page, roomId, bobName, bob.userId);

  // Alice sender filen — forsøger P2P, men Bob svarer ikke
  // → DataChannel åbner aldrig → 10s fallback → backend-besked
  await alice.page.setInputFiles('#fileInput', TEST_FILE);

  await alice.page.waitForSelector('.file-link', { timeout: 15000 });
  await expect(alice.page.locator('.file-link')).toContainText('testfil.txt');

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
});
