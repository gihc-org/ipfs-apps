# 0028 — Loft: mesh-topologi og per-deltager-signalering

**Status:** Draft (udkast — indstilles til `~/projects/adrs` ved accept)
**Dato:** 2026-09-09
**Projekt:** Loft (ipfs-apps)

## Kontekst

Loft er gruppe-link-rum med lyd, video og skærmdeling. Eksisterende WebRTC-kode
er bygget til 1:1 (DM): én global `RTCPeerConnection` i chat.html, signaler
med `target`-felt der routes via per-bruger kanaler i backenden (ADR-0016),
og call-flow med invite/accept/reject (ADR-0018). Gruppe-rum kræver at hver
deltager har en peer-forbindelse til alle andre (mesh), og at deltagerlisten er
synkron mellem klienterne.

Mesh fungerer til små grupper; SFU (Selective Forwarding Unit) er nødvendig
når antallet vokser. ADR-0016 udskød gruppe-topologi til en separat ADR.

## Beslutning

- **Mesh-topologi nu:** hver deltager holder ét `RTCPeerConnection` pr. anden
  deltager. Serveren er en dum signal-relay og ser aldrig media.
- **Server-side deltager-identitet:** når en klient joiner via WS, tildeler
  serveren en deltager-id og tilføjer afsenderen på alle signal-beskeder —
  klienter kan ikke forfalske `from`.
- **Signalering pr. deltager:** beskeder med `to: <deltager-id>` routes til
  modtagerens personlige kanal (mekanismen i `user_senders` genbruges).
- **Roster/presence:** ved join sender serveren den fulde deltagerliste
  (`roster`); `join`/`leave`/`presence` broadcastes til rummet, så klienter
  kan oprette/lukke peer-forbindelser deterministisk.
- **Perfect negotiation** i klienten: samme `RTCPeerConnection` bruges til
  lyd, video og skærmdeling med dynamisk add/remove af tracks (refaktor af
  chat.html's ene PC til et `rtc.js`-mesh-modul).
- **Praktisk loft på deltagere:** ~6 deltagere som guideline for mesh. Når
  det ikke slår til, skrives en ny ADR om SFU (fx LiveKit).

## Konsekvenser

- Backend-WS udvides med `roster` og media-state-beskeder; chat- og
  fil-beskeder fjernes.
- Klienten refaktoreres fra én global PC til et per-peer-kort med
  renegotiation — den største frontend-opgave i M2.
- TURN bliver vigtigere med flere deltagere bag NAT — coturn beholdes.
- Opkaldshistorik gemmes ikke; GDPR-overfladen forbliver minimal.
