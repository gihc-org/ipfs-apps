// Loft — lokal udvikling.
// I k8s overskrives denne fil af ConfigMap'en loft-config (config.js), som
// peger på miljøets origin (fx https://loft.test.gihc.online).

// Kører backenden direkte med cargo run lytter den på 8080. Kører du via
// docker compose er porten 8001 (videresendt til 8080 i containeren).
const API_URL = 'http://localhost:8080';
const WS_URL = 'ws://localhost:8080';

// TURN er valgfrit lokalt — med tom secret springes TURN over (STUN bruges).
// I prod sættes TURN_URL + TURN_SECRET af loft-config (se k8s/test).
const TURN_URL = '';
const TURN_SECRET = '';
