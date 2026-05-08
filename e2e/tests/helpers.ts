import { Page } from '@playwright/test';

export const API_URL = process.env.TEST_API_URL ?? 'http://localhost:8001/v1';
const WS_URL = API_URL.replace('https://', 'wss://').replace('http://', 'ws://');

// Intercept config.js so the frontend points to the test API, not prod.
export async function mockConfig(page: Page) {
  await page.route('**/config.js', route => route.fulfill({
    contentType: 'application/javascript',
    body: `
      const API_URL = '${API_URL}';
      const WS_URL  = '${WS_URL}';
      const TURNSTILE_SITE_KEY = '';
    `,
  }));
}

export async function register(page: Page, username: string, password: string) {
  // Bypass UI to avoid Turnstile — backend skips CAPTCHA when TURNSTILE_SECRET is empty.
  await page.request.post(`${API_URL}/auth/register`, {
    data: { username, password, email: `${username}@test.example`, turnstile_token: 'test' },
  });
}

export async function login(page: Page, username: string, password: string) {
  await mockConfig(page);
  await page.goto('/');
  await page.fill('#loginUsername', username);
  await page.fill('#loginPassword', password);
  await page.click('#loginForm button[type=submit]');
  await page.waitForURL('**/rooms.html');
}

export function uniqueUser(prefix = 'user') {
  return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
}
