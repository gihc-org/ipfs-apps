import { Page } from '@playwright/test';

export async function register(page: Page, username: string, password: string) {
  // Bypass UI registration to avoid Turnstile in headless mode.
  // Backend skips CAPTCHA validation when TURNSTILE_SECRET is empty (local dev).
  await page.request.post('/auth/register', {
    data: { username, password, email: `${username}@test.example`, turnstile_token: 'test' },
  });
}

export async function login(page: Page, username: string, password: string) {
  await page.goto('/');
  await page.fill('#loginUsername', username);
  await page.fill('#loginPassword', password);
  await page.click('#loginForm button[type=submit]');
  await page.waitForURL('**/rooms.html');
}

export function uniqueUser() {
  return `user_${Date.now()}`;
}
