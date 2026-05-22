import { test, expect, Browser, Page } from '@playwright/test';
import { register, login, mockConfig, uniqueUser, deleteUser, API_URL } from './helpers';

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

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
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

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
});

test('RTC-status pill, logging og rtcDump() under opkald', async ({ browser }) => {
  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  const dmRes = await alice.page.request.post(`${API_URL}/dms`, {
    headers: { Authorization: `Bearer ${alice.token}` },
    data: { user_id: bob.userId },
  });
  if (!dmRes.ok()) throw new Error(`DM POST failed ${dmRes.status()}: ${await dmRes.text()}`);
  const { room_id: roomId } = JSON.parse(await dmRes.text());

  const aliceName = await alice.page.evaluate(() => localStorage.getItem('username') ?? '');
  const bobName   = await bob.page.evaluate(()   => localStorage.getItem('username') ?? '');

  // Capture [webrtc]-logs på Alice
  const aliceLogs: string[] = [];
  alice.page.on('console', msg => {
    const text = msg.text();
    if (text.startsWith('[webrtc')) aliceLogs.push(text);
  });

  await openDm(alice.page, roomId, bobName,   bob.userId);
  await openDm(bob.page,   roomId, aliceName, alice.userId);

  // Inden opkald: pillen er skjult
  await expect(alice.page.locator('#rtcStatus')).not.toHaveClass(/visible/);

  // Alice ringer op, Bob accepterer
  await alice.page.waitForSelector('#callBtn', { state: 'visible' });
  await alice.page.click('#callBtn');
  await bob.page.waitForSelector('#incomingCall.visible', { timeout: 10000 });
  await bob.page.click('button:has-text("Accepter")');
  await alice.page.waitForSelector('#activeCall.visible', { timeout: 15000 });

  // Pillen er synlig og har RTC:/ICE:-tekst
  const pill = alice.page.locator('#rtcStatus');
  await expect(pill).toHaveClass(/visible/, { timeout: 5000 });
  await expect(pill).toContainText(/RTC: \S+ · ICE: \S+/, { timeout: 5000 });

  // Logs indeholder forventede events
  await expect.poll(() => aliceLogs.join('\n'), { timeout: 5000 })
    .toContain('call-start');
  expect(aliceLogs.some(l => l.includes('pc-created'))).toBe(true);
  expect(aliceLogs.some(l => l.includes('offer-sent'))).toBe(true);
  expect(aliceLogs.some(l => l.includes('signaling-state'))).toBe(true);

  // rtcDump returnerer øjebliksbillede med forventet shape
  const dump = await alice.page.evaluate(() => (window as any).rtcDump());
  expect(dump).toBeTruthy();
  expect(dump).toHaveProperty('connectionState');
  expect(dump).toHaveProperty('iceConnectionState');
  expect(dump).toHaveProperty('signalingState');
  expect(dump.flags).toMatchObject({ inCall: true });
  expect(Array.isArray(dump.senders)).toBe(true);
  expect(dump.senders.some((s: { kind: string }) => s.kind === 'audio')).toBe(true);

  // Læg på → pillen forsvinder igen
  await alice.page.click('button:has-text("Læg på")');
  await expect(alice.page.locator('#activeCall')).not.toHaveClass(/visible/, { timeout: 5000 });
  await expect(pill).not.toHaveClass(/visible/, { timeout: 5000 });

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
});

test('remoteVideo.srcObject overlever transient disconnect, ryddes ved failed', async ({ browser }) => {
  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  // Wrap RTCPeerConnection på Bob så vi kan tilgå PC-instansen i page.evaluate
  await bob.page.addInitScript(() => {
    const Original = window.RTCPeerConnection;
    (window as any).__pcs = [];
    window.RTCPeerConnection = new Proxy(Original, {
      construct(target, args) {
        const instance = Reflect.construct(target, args, target);
        (window as any).__pcs.push(instance);
        return instance;
      },
    }) as any;
  });

  const dmRes = await alice.page.request.post(`${API_URL}/dms`, {
    headers: { Authorization: `Bearer ${alice.token}` },
    data: { user_id: bob.userId },
  });
  if (!dmRes.ok()) throw new Error(`DM POST failed ${dmRes.status()}: ${await dmRes.text()}`);
  const { room_id: roomId } = JSON.parse(await dmRes.text());

  const aliceName = await alice.page.evaluate(() => localStorage.getItem('username') ?? '');
  const bobName   = await bob.page.evaluate(()   => localStorage.getItem('username') ?? '');

  await openDm(alice.page, roomId, bobName,   bob.userId);
  await openDm(bob.page,   roomId, aliceName, alice.userId);

  // Tving en PC til at blive oprettet på Bob (callee-rolle ved opkald → handleSignal('offer') opretter PC)
  await alice.page.waitForSelector('#callBtn', { state: 'visible' });
  await alice.page.click('#callBtn');
  await bob.page.waitForSelector('#incomingCall.visible', { timeout: 10000 });
  await bob.page.click('button:has-text("Accepter")');
  await bob.page.waitForSelector('#activeCall.visible', { timeout: 15000 });
  await bob.page.waitForFunction(() => (window as any).__pcs.length > 0, { timeout: 5000 });

  // Simulér at Bob modtager screen-share — injicér en fake MediaStream i #remoteVideo
  await bob.page.evaluate(() => {
    const video = document.getElementById('remoteVideo') as HTMLVideoElement;
    video.srcObject = new MediaStream();
  });

  const hasStream = () => bob.page.evaluate(() =>
    !!(document.getElementById('remoteVideo') as HTMLVideoElement).srcObject
  );
  expect(await hasStream()).toBe(true);

  // Syntetisér transient disconnect: skift connectionState til 'disconnected' og fyr event
  await bob.page.evaluate(() => {
    const pc = (window as any).__pcs.at(-1);
    Object.defineProperty(pc, 'connectionState', { get: () => 'disconnected', configurable: true });
    pc.dispatchEvent(new Event('connectionstatechange'));
  });

  // Fix verificeret: srcObject overlever transient disconnect
  expect(await hasStream()).toBe(true);
  await expect(bob.page.locator('#rtcStatus')).toHaveAttribute('data-state', 'disconnected');

  // Permanent failed: closePeerVideo SKAL stadig fyre
  await bob.page.evaluate(() => {
    const pc = (window as any).__pcs.at(-1);
    Object.defineProperty(pc, 'connectionState', { get: () => 'failed', configurable: true });
    pc.dispatchEvent(new Event('connectionstatechange'));
  });

  await expect.poll(hasStream, { timeout: 2000 }).toBe(false);

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
});

test('rtcDump() returnerer null når intet PC findes', async ({ browser }) => {
  const pw = 'Password123!';
  const alice = await newSession(browser, uniqueUser('alice'), pw);
  const bob   = await newSession(browser, uniqueUser('bob'),   pw);

  const dmRes = await alice.page.request.post(`${API_URL}/dms`, {
    headers: { Authorization: `Bearer ${alice.token}` },
    data: { user_id: bob.userId },
  });
  if (!dmRes.ok()) throw new Error(`DM POST failed ${dmRes.status()}: ${await dmRes.text()}`);
  const { room_id: roomId } = JSON.parse(await dmRes.text());

  const bobName = await bob.page.evaluate(() => localStorage.getItem('username') ?? '');
  await openDm(alice.page, roomId, bobName, bob.userId);

  const dump = await alice.page.evaluate(() => (window as any).rtcDump());
  expect(dump).toBeNull();
  await expect(alice.page.locator('#rtcStatus')).not.toHaveClass(/visible/);

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
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

  await deleteUser(alice.page, alice.token);
  await deleteUser(bob.page, bob.token);
});

