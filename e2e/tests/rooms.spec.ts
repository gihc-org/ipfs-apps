import { test, expect } from '@playwright/test';
import { register, login, uniqueUser } from './helpers';

test.beforeEach(async ({ page }) => {
  const username = uniqueUser();
  await register(page, username, 'Password123!');
  await login(page, username, 'Password123!');
});

test('opret rum vises i listen', async ({ page }) => {
  const roomName = `rum-${Date.now()}`;
  await page.fill('#roomName', roomName);
  await page.click('button:has-text("Opret")');
  await expect(page.locator('.room-card', { hasText: roomName })).toBeVisible();
});

test('duplikeret rum giver fejlbesked', async ({ page }) => {
  const roomName = `rum-${Date.now()}`;
  await page.fill('#roomName', roomName);
  await page.click('button:has-text("Opret")');
  // Vent på at første rum er oprettet før vi prøver at oprette det igen
  await page.locator('.room-card', { hasText: roomName }).waitFor();
  await page.fill('#roomName', roomName);
  await page.click('button:has-text("Opret")');
  await expect(page.locator('#createError')).not.toBeEmpty();
});

test('klik på rum åbner chat', async ({ page }) => {
  const roomName = `rum-${Date.now()}`;
  await page.fill('#roomName', roomName);
  await page.click('button:has-text("Opret")');
  await page.locator('.room-card', { hasText: roomName }).click();
  await expect(page).toHaveURL(/chat\.html/);
});
