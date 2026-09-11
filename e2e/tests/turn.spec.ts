import { createHmac } from 'crypto';
import { BrowserContext, Page, expect, test } from '@playwright/test';

// TURN-relay-test: beviser at coturn faktisk kan relaye media — den del af
// stakken som STUN-only-tests ikke dækker, og som er den der fejler bag NAT.
//
// Testen kaldes med miljøvariabler, så den kan køre mod ethvert miljø (og
// springes over i CI, hvor der ikke er nogen TURN-server):
//
//   TURN_URL='turn:loft.test.gihc.online:3478?transport=udp' \
//   TURN_SECRET="$(kubectl -n loft-test get cm loft-config -o jsonpath='{.data.turn-secret}')" \
//   npx playwright test tests/turn.spec.ts
//
// To selvstændige browser-kontekster forbinder med `iceTransportPolicy: 'relay'`,
// så alle kandidater kommer fra coturn. Data-kanalen går derfor gennem
// relayeren, og testen fejler hvis allocation, permissions eller datavejen er i
// stykker. Kandidattyper og -fejl rapporteres, så en fejl kan forklares.

const TURN_URL = process.env.TURN_URL;
const TURN_SECRET = process.env.TURN_SECRET;

test.skip(!TURN_URL || !TURN_SECRET, 'sæt TURN_URL og TURN_SECRET for at teste TURN-relay');

// Relay-opsætningen tager tid (allocation på coturn + ICE-gennemløb).
test.setTimeout(120_000);

function turnCredentials(secret: string, ttlSeconds = 3600) {
  const username = String(Math.floor(Date.now() / 1000) + ttlSeconds);
  const credential = createHmac('sha1', secret).update(username).digest('base64');
  return { username, credential };
}

/** Sætter en relay-only peer op i siden og eksponerer hjælpefunktioner. */
async function setupPeer(page: Page, iceServer: { urls: string; username: string; credential: string }) {
  await page.goto('/');
  await page.evaluate((ice) => {
    const state: any = { received: [], candidates: [], errors: [], channelState: 'ingen' };
    const pc = new RTCPeerConnection({ iceServers: [ice], iceTransportPolicy: 'relay' });
    state.pc = pc;
    pc.addEventListener('icecandidate', (event) => {
      if (event.candidate) state.candidates.push(event.candidate.type ?? 'ukendt');
    });
    pc.addEventListener('icecandidateerror', (event: any) => {
      state.errors.push(`${event.errorCode}: ${event.errorText}`);
    });
    pc.addEventListener('connectionstatechange', () => {
      state.connectionState = pc.connectionState;
    });
    state.setChannel = (channel: RTCDataChannel) => {
      state.channel = channel;
      state.channelState = channel.readyState;
      channel.addEventListener('open', () => {
        state.channelState = 'open';
      });
      channel.addEventListener('message', (message) => state.received.push(String(message.data)));
    };
    state.waitIce = (pc2: RTCPeerConnection) =>
      new Promise<void>((resolve) => {
        if (pc2.iceGatheringState === 'complete') return resolve();
        const timer = setTimeout(resolve, 15000);
        pc2.addEventListener('icegatheringstatechange', () => {
          if (pc2.iceGatheringState === 'complete') {
            clearTimeout(timer);
            resolve();
          }
        });
      });
    (window as any).__turn = state;
  }, iceServer);
}

async function newPage(context: BrowserContext): Promise<Page> {
  const page = await context.newPage();
  return page;
}

test('TURN relayer media mellem to klienter (relay-only ICE)', async ({ browser }) => {
  const iceServer = { urls: TURN_URL!, ...turnCredentials(TURN_SECRET!) };
  const aliceContext = await browser.newContext();
  const bobContext = await browser.newContext();
  const alice = await newPage(aliceContext);
  const bob = await newPage(bobContext);

  await setupPeer(alice, iceServer);
  await setupPeer(bob, iceServer);

  // Bob tager imod kanalen; Alice opretter den.
  await bob.evaluate(() => {
    (window as any).__turn.pc.ondatachannel = (event: RTCDataChannelEvent) =>
      (window as any).__turn.setChannel(event.channel);
  });
  await alice.evaluate(() => {
    (window as any).__turn.setChannel((window as any).__turn.pc.createDataChannel('relay-probe'));
  });

  const offer = await alice.evaluate(async () => {
    const state = (window as any).__turn;
    await state.pc.setLocalDescription(await state.pc.createOffer());
    await state.waitIce(state.pc);
    return state.pc.localDescription;
  });
  const answer = await bob.evaluate(async (remote: RTCSessionDescriptionInit) => {
    const state = (window as any).__turn;
    await state.pc.setRemoteDescription(remote);
    await state.pc.setLocalDescription(await state.pc.createAnswer());
    await state.waitIce(state.pc);
    return state.pc.localDescription;
  }, offer);
  await alice.evaluate(async (remote: RTCSessionDescriptionInit) => {
    await (window as any).__turn.pc.setRemoteDescription(remote);
  }, answer);

  const diagnostics = () =>
    Promise.all(
      [alice, bob].map((page) =>
        page.evaluate(() => {
          const s = (window as any).__turn;
          return {
            connectionState: s.connectionState,
            channelState: s.channelState,
            candidates: s.candidates,
            errors: s.errors.slice(0, 3),
            received: s.received,
          };
        }),
      ),
    );

  let bothOpen = true;
  try {
    for (const page of [alice, bob]) {
      await page.waitForFunction(() => (window as any).__turn.channelState === 'open', null, {
        timeout: 30000,
      });
    }
  } catch {
    bothOpen = false;
  }
  expect(bothOpen, `data-kanalen åbnede ikke: ${JSON.stringify(await diagnostics())}`).toBe(true);

  // Begge retninger gennem relayeren.
  await alice.evaluate(() => (window as any).__turn.channel.send('alice-til-bob'));
  await bob.waitForFunction(() => (window as any).__turn.received.includes('alice-til-bob'), null, {
    timeout: 20000,
  });
  await bob.evaluate(() => (window as any).__turn.channel.send('bob-til-alice'));
  await alice.waitForFunction(() => (window as any).__turn.received.includes('bob-til-alice'), null, {
    timeout: 20000,
  });

  const [aliceState, bobState] = await diagnostics();
  for (const state of [aliceState, bobState]) {
    expect(state.candidates.length).toBeGreaterThan(0);
    expect(state.candidates.every((type) => type === 'relay')).toBe(true);
  }
  console.log(
    `  relay ok: alice=${aliceState.candidates.join(',')} bob=${bobState.candidates.join(',')}`,
  );

  await aliceContext.close();
  await bobContext.close();
});
