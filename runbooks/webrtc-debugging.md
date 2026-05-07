# WebRTC fejlfinding

## Arkitektur (fase 1)

```
Alice                  Server (WS)               Bob
  │── signal(offer) ──▶│── broadcast ───────────▶│
  │◀─────────────────── │◀── signal(answer) ──────│
  │    [ICE-kandidater udveksles begge veje]      │
  │                                               │
  └──────── direkte peer-to-peer mediaforbindelse ┘
                  (skærm-stream)
```

Serveren ser kun krypterede signalerings-beskeder. Selve mediestrømmen
går direkte mellem browserne via STUN (`stun.l.google.com:19302`,
`stun.cloudflare.com:3478`).

## "Del skærm"-knappen er ikke synlig

Knappen vises kun i DM-rum med en `peer_id` i URL'en. Tjek:

1. Er URL'en korrekt? Skal indeholde `is_dm=true&peer_id=<uuid>`
2. Er `userId` gemt i `localStorage`? Åbn DevTools → Application →
   Local Storage → tjek at `userId` er sat (sættes ved login via rooms.html)
3. Prøv at gå tilbage til rooms.html og ind i DM-rummet igen

## Forbindelsen oprettes aldrig (ingen stream hos modtager)

**Tjek WebSocket-signalering:**

Åbn DevTools → Network → WS → find `/ws/<room_id>` → Messages-fanen.
Bekræft at offer, answer og ICE-kandidater udveksles.

**Tjek browser-konsollen for fejl:**

Typiske fejl:
- `NotAllowedError` — brugeren afslog skærmdelingstilladelse
- `InvalidStateError` — `RTCPeerConnection` er i forkert tilstand (prøv reload)
- `Failed to set remote description` — SDP-format uoverensstemmelse

**STUN fejler (symmetrisk NAT):**

Hvis begge parter er bag restriktive firewalls, kan den direkte
peer-forbindelse ikke oprettes. Tegn:
- ICE-kandidater udveksles i WS (ses i Network-fanen)
- Men `pc.connectionState` forbliver `"checking"` og går aldrig til `"connected"`

Løsning: tilføj en TURN-server (se ADR-0016, fase 2).

## Stream stopper uventet

- Brugeren har lukket browser-fanen med skærmdelings-indikatoren
- WS-forbindelsen er tabt og genoprettet (siden reconnecter automatisk
  men WebRTC-forbindelsen lukkes ved WS-disconnect)
- `pc.connectionState` skiftede til `"disconnected"` eller `"failed"`

Løsning: klik "Del skærm" igen for at starte en ny session.

## Lokal test med to browserfaner

1. Log ind som bruger A i fane 1
2. Log ind som bruger B i fane 2 (brug privat/inkognito-tilstand)
3. Opret et DM-rum mellem A og B fra rooms.html i begge faner
4. Åbn DM-rummet i begge faner — "Del skærm"-knappen vises
5. Klik "Del skærm" i fane 1, vælg en fane eller et vindue
6. Stream vises i fane 2 inden for få sekunder

## Backend-validering

Signal-beskeder valideres af backenden:
- `target` skal være en gyldig UUID — ugyldige targets droppes lydløst
- `from` sættes af serveren til den autentificerede brugers UUID —
  klienten kan ikke forfalske afsender

Integrationstests dækker disse tilfælde: `signal_forwarded_with_server_stamped_from`,
`signal_with_invalid_target_is_dropped`, `signal_is_not_persisted_to_database`.
