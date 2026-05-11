import { test, expect } from '@playwright/test';
import { register, login, uniqueUser, deleteUser } from './helpers';

test('redirect til rooms efter login', async ({ page }) => {
  const username = uniqueUser();
  await register(page, username, 'Password123!');
  await login(page, username, 'Password123!');
  await expect(page).toHaveURL(/rooms\.html/);
  await expect(page.locator('#navUser')).toHaveText(username);
  await deleteUser(page);
});

test('fejlbesked ved forkert adgangskode', async ({ page }) => {
  await page.goto('/');
  await page.fill('#loginUsername', 'ikkeeksisterende');
  await page.fill('#loginPassword', 'forkert');
  await page.click('#loginForm button[type=submit]');
  await expect(page.locator('#loginError')).not.toBeEmpty();
});

test('redirect til login ved stale token', async ({ page }) => {
  await page.goto('/rooms.html');
  // Uden token redirect til index
  await expect(page).toHaveURL(/index\.html/);
});

test('logout rydder session og redirecter', async ({ page }) => {
  const username = uniqueUser();
  await register(page, username, 'Password123!');
  await login(page, username, 'Password123!');
  const token = await page.evaluate(() => localStorage.getItem('token') ?? '');
  await page.click('button:has-text("Log ud")');
  await expect(page).toHaveURL(/index\.html/);
  // Token er væk — rooms.html redirecter tilbage til index
  await page.goto('/rooms.html');
  await expect(page).toHaveURL(/index\.html/);
  await deleteUser(page, token);
});
