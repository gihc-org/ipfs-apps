import { createHmac } from 'crypto';
import { BrowserContext, Page, expect, test } from '@playwright/test';

// TURN-relay-test: beviser at coturn faktisk kan relaye media — den del af
// stakken som STUN-only-tests ikke dækker, og som er den der fejler bag NAT.
//
// Testen kaldes med miljøvariabler, så den kan køre mod ethvert miljø (og
// springes over i CI, hvor der ikke er nogen TURN-server):
//
//   TURN_URL="$(~/projects/infra/scripts/turn-config.sh --url)" \
//   TURN_SECRET="$(pass turn/static-auth-secret)" \
//   npx playwright test tests/turn.spec.ts
//
// TURN er platformens delte instans (turn.gihc.online, ADR 0003 i infra) —
// appen har ingen egen coturn, og hemmeligheden kommer fra `pass`.
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
      // Firefox kan sende en afsluttende, tom kandidat (ingen candidate-streng/
      // adresse). Den er ikke en rigtig kandidat og tælles ikke med.
      const candidate = event.candidate;
      if (!candidate || !candidate.candidate) return;
      state.candidates.push(candidate.type ?? 'ukendt');
    });
    // Den valgte kandidatpair er det egentlige bevis: den siger hvilken vej
    // mediet faktisk gik, uafhængigt af hvordan browseren rapporterer
    // kandidat-events (Firefox og Chromium gør det forskelligt).
    state.selectedPair = async () => {
      const stats = await pc.getStats();
      let pair: any = null;
      stats.forEach((report: any) => {
        if (report.type === 'candidate-pair' && report.state === 'succeeded' &&
            (report.nominated || report.selected)) {
          pair = report;
        }
      });
      if (!pair) return null;
      const local = stats.get(pair.localCandidateId);
      const remote = stats.get(pair.remoteCandidateId);
      return {
        localType: local?.candidateType ?? local?.type ?? null,
        remoteType: remote?.candidateType ?? remote?.type ?? null,
      };
    };
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

  // Og beviset på at mediet faktisk gik gennem relayeren: den valgte
  // kandidatpair skal være af typen relay i begge ender.
  const pairs: ({ localType: string | null; remoteType: string | null } | null)[] = [];
  for (const page of [alice, bob]) {
    await page.waitForFunction(
      () => (window as any).__turn.pc.connectionState === 'connected',
      null,
      { timeout: 20000 },
    );
    pairs.push(await page.evaluate(() => (window as any).__turn.selectedPair()));
  }
  for (const pair of pairs) {
    expect(pair, 'fandt ingen valgt kandidatpair').not.toBeNull();
    expect(pair!.localType, `valgt lokal kandidat var ${pair!.localType}`).toBe('relay');
  }

  console.log(
    `  relay ok: alice=${aliceState.candidates.join(',')} bob=${bobState.candidates.join(',')}` +
    ` (valgt pair: ${pairs.map((p) => `${p!.localType}/${p!.remoteType}`).join(', ')})`,
  );

  await aliceContext.close();
  await bobContext.close();
});

// Testen ovenfor bruger credentials fra miljøvariabler. Denne bruger i stedet
// den DEPLOYEDE sides egen config.js — altså hele kæden config.js →
// loft.html's buildIceServers() → RTCPeerConnection. Den kæde er ellers
// udækket: de øvrige loft-tests stubber config.js med tom TURN, så en forkert
// TURN_URL (fx den gamle loft.test-host) ville først vise sig for en bruger.
test('den deployede sides egen TURN-config giver en relay-kandidat', async ({ page }) => {
  test.skip(!process.env.BASE_URL, 'kræver et deployet miljø (BASE_URL)');

  await page.goto('/');

  const iceServers: { urls: string; username?: string; credential?: string }[] = await page.evaluate(
    () => (window as any).buildIceServers(),
  );
  const turn = iceServers.find((server) => server.urls.startsWith('turn:'));
  expect(turn, `ingen TURN-server i sidens config.js: ${JSON.stringify(iceServers)}`).toBeTruthy();
  expect(turn!.username, 'TURN uden brugernavn — er TURN_SECRET sat i config.js?').toBeTruthy();
  expect(turn!.credential, 'TURN uden credential — er TURN_SECRET sat i config.js?').toBeTruthy();

  const candidates: string[] = await page.evaluate(async () => {
    const servers = await (window as any).buildIceServers();
    const pc = new RTCPeerConnection({ iceServers: servers, iceTransportPolicy: 'relay' });
    const types: string[] = [];
    pc.addEventListener('icecandidate', (event) => {
      if (event.candidate) types.push(event.candidate.type ?? 'ukendt');
    });
    pc.createDataChannel('side-config-probe');
    await pc.setLocalDescription(await pc.createOffer());
    await new Promise<void>((resolve) => {
      if (pc.iceGatheringState === 'complete') return resolve();
      const timer = setTimeout(resolve, 15000);
      pc.addEventListener('icegatheringstatechange', () => {
        if (pc.iceGatheringState === 'complete') {
          clearTimeout(timer);
          resolve();
        }
      });
    });
    pc.close();
    return types;
  });

  const relay = candidates.filter((type) => type === 'relay');
  expect(relay.length, `ingen relay-kandidat fra ${turn!.urls} (kandidater: ${candidates.join(',') || 'ingen'})`).toBeGreaterThan(0);
  console.log(`  side-config ok: ${turn!.urls} → ${relay.length} relay-kandidat(er)`);
});
