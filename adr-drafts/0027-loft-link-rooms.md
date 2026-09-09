# 0027 — Loft: link-rum med gæsteadgang på k3s

**Status:** Draft (udkast — indstilles til `~/projects/adrs` ved accept)
**Dato:** 2026-09-09
**Projekt:** ipfs-apps → Loft

## Kontekst

Chat-appen har vokset et generelt chat-produkt (rum, DM, filoverførsel,
konti med email-verifikation) oven på en WebRTC-kerne. Projektet skal i stedet
fokuseres til ét scenarie: man opretter et link-rum, deler URL'en via Matrix,
XMPP eller mail, og modtageren joiner med lyd/video/skærmdeling uden at
oprette en konto. Samtidig migreres appen fra en Caddy + Docker Compose-stak
til k3s, hvor platformen allerede er klar og der ingen data er at migrere.

"Plan B"-modellen: rummet lever videre efter at skaberen forlader det, indtil
det lukkes eller udløber. Det passer til deling over mail, hvor modtageren
typisk åbner linket på et andet tidspunkt end afsenderen.

## Beslutning

- **Loft er et link-rum med gæsteadgang.** Loft-URL'en (tilfældig UUID) er
  adgangsnøgle; der er ingen konti, JWT, email-verifikation eller CAPTCHA i
  MVP. Deltageren vælger selv et vist navn ved join.
- **Frontend serveres som statisk nginx-image** pr. miljø med `config.js`
  mountet som ConfigMap. IPFS/DNSLink og kubo udgår.
- **Ét origin pr. miljø:** frontend, REST (`/v1`) og WebSocket deler host
  (`loft.test.gihc.online`), så CORS/CSP-par-koblingen forsvinder.
- **En lille PostgreSQL-instans pr. miljø-namespace** gemmer kun lofts
  (id, navn, created_at, last_active). Deltagere, presence og signalering er
  in-memory → én replica.
- **coturn kører som `hostNetwork`-Deployment** med firewall-regler i
  `infra/tofu` (TCP+UDP 3478, UDP 49152–49200).
- **CI via GitHub Actions → GHCR** (`ghcr.io/gihc-org/loft` og `loft-web`)
  med SHA-tags. Secrets fra `pass` → `kubectl create secret`.

## Alternativer overvejet

- **Behold IPFS/DNSLink:** bevarer projektets IPFS-identitet, men tilføjer
  kubo-pod, PVC og DNSLink-deploy i CI — kompleksitet uden produktværdi for et
  link-rum. Vælges fra.
- **Live-loft (Slack-model):** rummet lukker når sidste deltager går. Giver
  minimal persistens (endda ingen postgres), men gør mail-deling ubrugelig.
- **Konti/JWT:** genbruger eksisterende auth-stak, men giver friktion ved
  link-deling til folk uden konto og øger GDPR-overfladen. Udskydes til fase 2.

## Konsekvenser

- Chat-rum, DM, filoverførsel, uploads og auth-kode fjernes fra den aktive
  sti i M1–M5; git-historik bevares.
- ADR-0008 (IPFS + DNSLink) og platform-ADR'erne for Caddy/Ansible (0009,
  0010, 0019) afløses for dette produkt af denne beslutning.
- `/healthz` og containerhærdning (non-root, read-only fs) bliver krav før
  k8s-deploy.
- WS-håndtryk ændres fra JWT-validering til capability-baseret adgang.
- Nyt domæneskema kræver DNS-records og senere oprydning af de gamle.
