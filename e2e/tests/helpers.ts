import { Page } from '@playwright/test';

// Lokalt forventes backenden at køre (fx PORT=8081) — pej på origin med
// TEST_API_URL (med eller uden /v1). loft.html appender selv /v1.
export const API_ORIGIN = (process.env.TEST_API_URL ?? 'http://localhost:8080').replace(/\/v1$/, '');
export const API_URL = `${API_ORIGIN}/v1`;
const WS_URL = API_ORIGIN.replace(/^http/, 'ws');

// Overskriver config.js så frontenden peger på test-API'et, ikke prod.
export async function mockConfig(page: Page) {
  await page.route('**/config.js', (route) =>
    route.fulfill({
      contentType: 'application/javascript',
      body: `
        const API_URL = '${API_ORIGIN}';
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
