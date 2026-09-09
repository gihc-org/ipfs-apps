import { Page } from '@playwright/test';

// Lokalt forventes backenden at køre (fx PORT=8081) — pej på den med
// TEST_API_URL. I CI/deploy peges der på miljøets origin.
export const API_URL = (process.env.TEST_API_URL ?? 'http://localhost:8080/v1').replace(/\/$/, '');
const API_BASE = API_URL.replace(/\/v1$/, '');
const WS_URL = API_BASE.replace(/^http/, 'ws');

// Overskriver config.js så frontenden peger på test-API'et, ikke prod.
export async function mockConfig(page: Page) {
  await page.route('**/config.js', (route) =>
    route.fulfill({
      contentType: 'application/javascript',
      body: `
        const API_URL = '${API_URL}';
        const WS_URL = '${WS_URL}';
        const TURN_URL = '';
        const TURN_SECRET = '';
      `,
    }),
  );
}

export function uniqueName(prefix = 'deltager') {
  return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
}
